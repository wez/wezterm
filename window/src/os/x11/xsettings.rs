use anyhow::Context;
/// This module parses xsettings data.
/// The data format is slightly incorrectly documented here:
/// <https://specifications.freedesktop.org/xsettings-spec/xsettings-latest.html>
/// I looked at the libxsettings-client sources to verify the behavior;
/// there is a discrepancy in the representation of string setting lengths,
/// but otherwise it seems to parse the data from my 2021 gnome window environment.
use bytes::Buf;
use std::collections::BTreeMap;
use xcb::x::Atom;

pub type XSettingsMap = BTreeMap<String, XSetting>;

#[derive(Debug, PartialEq, Eq)]
pub enum XSetting {
    Integer(i32),
    String(String),
    Color(u16, u16, u16, u16),
}

fn read_xsettings_grabbed(
    conn: &xcb::Connection,
    atom_xsettings_selection: Atom,
    atom_xsettings_settings: Atom,
) -> anyhow::Result<XSettingsMap> {
    let manager = conn
        .wait_for_reply(conn.send_request(&xcb::x::GetSelectionOwner {
            selection: atom_xsettings_selection,
        }))?
        .owner();

    let reply = conn
        .wait_for_reply(conn.send_request(&xcb::x::GetProperty {
            delete: false,
            window: manager,
            property: atom_xsettings_settings,
            r#type: atom_xsettings_settings,
            long_offset: 0,
            long_length: u32::max_value(),
        }))
        .context("get_property")?;

    parse_xsettings(reply.value::<u8>())
}

pub fn read_xsettings(
    conn: &xcb::Connection,
    atom_xsettings_selection: Atom,
    atom_xsettings_settings: Atom,
) -> anyhow::Result<XSettingsMap> {
    conn.check_request(conn.send_request_checked(&xcb::x::GrabServer {}))
        .context("grab_server")?;
    let res = read_xsettings_grabbed(conn, atom_xsettings_selection, atom_xsettings_settings);
    conn.check_request(conn.send_request_checked(&xcb::x::UngrabServer {}))
        .context("ungrab_server")?;
    res
}

pub fn parse_xsettings(data: &[u8]) -> anyhow::Result<XSettingsMap> {
    anyhow::ensure!(data.len() > 0);

    let mut settings = BTreeMap::new();

    let mut buf = data;
    let is_big_endian = buf.get_u8() != 0;

    fn get_u8<BUF: Buf>(buf: &mut BUF) -> anyhow::Result<u8> {
        anyhow::ensure!(buf.remaining() >= 1);
        Ok(buf.get_u8())
    }
    fn get_u16<BUF: Buf>(buf: &mut BUF, is_big_endian: bool) -> anyhow::Result<u16> {
        anyhow::ensure!(buf.remaining() >= 2);
        Ok(if is_big_endian {
            buf.get_u16()
        } else {
            buf.get_u16_le()
        })
    }
    fn get_u32<BUF: Buf>(buf: &mut BUF, is_big_endian: bool) -> anyhow::Result<u32> {
        anyhow::ensure!(buf.remaining() >= 4);
        Ok(if is_big_endian {
            buf.get_u32()
        } else {
            buf.get_u32_le()
        })
    }
    fn get_i32<BUF: Buf>(buf: &mut BUF, is_big_endian: bool) -> anyhow::Result<i32> {
        anyhow::ensure!(buf.remaining() >= 4);
        Ok(if is_big_endian {
            buf.get_i32()
        } else {
            buf.get_i32_le()
        })
    }
    fn advance<BUF: Buf>(buf: &mut BUF, n: usize) -> anyhow::Result<()> {
        anyhow::ensure!(buf.remaining() >= n);
        buf.advance(n);
        Ok(())
    }

    advance(&mut buf, 3)?;
    let _serial = get_u32(&mut buf, is_big_endian)?;
    let num_settings = get_u32(&mut buf, is_big_endian)? as usize;

    fn pad(len: usize) -> usize {
        (len + 3) & !3
    }

    for _ in 0..num_settings {
        let setting_type = get_u8(&mut buf)?;
        advance(&mut buf, 1)?;
        let name_len = get_u16(&mut buf, is_big_endian)? as usize;
        let padded_name_len = pad(name_len);
        anyhow::ensure!(
            buf.remaining() >= padded_name_len,
            "name_len is {} (pad: {}) but buffer has {} remaining",
            name_len,
            padded_name_len,
            buf.remaining()
        );
        let name = String::from_utf8(buf.chunk()[..name_len].to_vec())?;
        buf.advance(padded_name_len);

        let _last_change_serial = get_u32(&mut buf, is_big_endian)?;

        let value = match setting_type {
            0 => XSetting::Integer(get_i32(&mut buf, is_big_endian)?),
            1 => {
                // Note that the xsettings-latest spec indicates that this
                // length is a u16, but that fails to parse real data,
                // and libxsettings-client treats it as u32, so we do too.
                let s_len = get_u32(&mut buf, is_big_endian)? as usize;
                let padded_s_len = pad(s_len);
                anyhow::ensure!(
                    buf.remaining() >= padded_s_len,
                    "s_len is {} (pad: {}) but buffer has {} remaining",
                    s_len,
                    padded_s_len,
                    buf.remaining()
                );
                let s = String::from_utf8(buf.chunk()[..s_len].to_vec())?;
                buf.advance(padded_s_len);

                XSetting::String(s)
            }
            2 => {
                let red = get_u16(&mut buf, is_big_endian)?;
                let green = get_u16(&mut buf, is_big_endian)?;
                let blue = get_u16(&mut buf, is_big_endian)?;
                let alpha = get_u16(&mut buf, is_big_endian)?;
                XSetting::Color(red, green, blue, alpha)
            }
            n => anyhow::bail!("invalid setting type {}, expected, 0, 1 or 2", n),
        };
        settings.insert(name, value);
    }

    Ok(settings)
}

mod test {
    #[test]
    fn test_parse_xsettings() {
        let data = [
            0, 0, 0, 0, 152, 0, 0, 0, 53, 0, 0, 0, 0, 0, 15, 0, 71, 100, 107, 47, 85, 110, 115, 99,
            97, 108, 101, 100, 68, 80, 73, 0, 0, 0, 0, 0, 0, 128, 1, 0, 0, 0, 22, 0, 71, 116, 107,
            47, 82, 101, 99, 101, 110, 116, 70, 105, 108, 101, 115, 69, 110, 97, 98, 108, 101, 100,
            0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 19, 0, 71, 116, 107, 47, 67, 117, 114, 115, 111,
            114, 84, 104, 101, 109, 101, 83, 105, 122, 101, 0, 0, 0, 0, 0, 24, 0, 0, 0, 0, 0, 23,
            0, 71, 116, 107, 47, 83, 104, 111, 119, 73, 110, 112, 117, 116, 77, 101, 116, 104, 111,
            100, 77, 101, 110, 117, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 0, 78, 101, 116, 47, 68,
            110, 100, 68, 114, 97, 103, 84, 104, 114, 101, 115, 104, 111, 108, 100, 0, 0, 0, 0, 8,
            0, 0, 0, 0, 0, 18, 0, 71, 116, 107, 47, 84, 105, 109, 101, 111, 117, 116, 73, 110, 105,
            116, 105, 97, 108, 0, 0, 0, 0, 0, 0, 200, 0, 0, 0, 1, 0, 20, 0, 71, 116, 107, 47, 68,
            101, 99, 111, 114, 97, 116, 105, 111, 110, 76, 97, 121, 111, 117, 116, 0, 0, 0, 0, 10,
            0, 0, 0, 109, 101, 110, 117, 58, 99, 108, 111, 115, 101, 0, 0, 0, 0, 13, 0, 88, 102,
            116, 47, 65, 110, 116, 105, 97, 108, 105, 97, 115, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
            0, 19, 0, 78, 101, 116, 47, 67, 117, 114, 115, 111, 114, 66, 108, 105, 110, 107, 84,
            105, 109, 101, 0, 0, 0, 0, 0, 176, 4, 0, 0, 1, 0, 12, 0, 71, 116, 107, 47, 73, 77, 77,
            111, 100, 117, 108, 101, 0, 0, 0, 0, 21, 0, 0, 0, 103, 116, 107, 45, 105, 109, 45, 99,
            111, 110, 116, 101, 120, 116, 45, 115, 105, 109, 112, 108, 101, 0, 0, 0, 0, 0, 21, 0,
            71, 116, 107, 47, 83, 104, 101, 108, 108, 83, 104, 111, 119, 115, 68, 101, 115, 107,
            116, 111, 112, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 11, 0, 88, 102, 116, 47, 72, 105,
            110, 116, 105, 110, 103, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 13, 0, 88, 102, 116, 47, 72,
            105, 110, 116, 83, 116, 121, 108, 101, 0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0, 104, 105, 110,
            116, 115, 108, 105, 103, 104, 116, 0, 0, 0, 0, 14, 0, 71, 116, 107, 47, 77, 101, 110,
            117, 73, 109, 97, 103, 101, 115, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 17, 0, 71, 116,
            107, 47, 84, 105, 109, 101, 111, 117, 116, 82, 101, 112, 101, 97, 116, 0, 0, 0, 0, 0,
            0, 0, 20, 0, 0, 0, 0, 0, 22, 0, 71, 116, 107, 47, 69, 110, 97, 98, 108, 101, 80, 114,
            105, 109, 97, 114, 121, 80, 97, 115, 116, 101, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 18,
            0, 71, 116, 107, 47, 75, 101, 121, 110, 97, 118, 85, 115, 101, 67, 97, 114, 101, 116,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 21, 0, 71, 116, 107, 47, 83, 104, 101, 108, 108,
            83, 104, 111, 119, 115, 65, 112, 112, 77, 101, 110, 117, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0, 19, 0, 71, 116, 107, 47, 67, 97, 110, 67, 104, 97, 110, 103, 101, 65, 99, 99,
            101, 108, 115, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 12, 0, 71, 116, 107, 47, 70, 111, 110,
            116, 78, 97, 109, 101, 0, 0, 0, 0, 12, 0, 0, 0, 67, 97, 110, 116, 97, 114, 101, 108,
            108, 32, 49, 49, 1, 0, 13, 0, 78, 101, 116, 47, 84, 104, 101, 109, 101, 78, 97, 109,
            101, 0, 0, 0, 0, 0, 0, 0, 12, 0, 0, 0, 65, 100, 119, 97, 105, 116, 97, 45, 100, 97,
            114, 107, 0, 0, 19, 0, 78, 101, 116, 47, 68, 111, 117, 98, 108, 101, 67, 108, 105, 99,
            107, 84, 105, 109, 101, 0, 0, 0, 0, 0, 144, 1, 0, 0, 0, 0, 20, 0, 71, 116, 107, 47, 68,
            105, 97, 108, 111, 103, 115, 85, 115, 101, 72, 101, 97, 100, 101, 114, 0, 0, 0, 0, 1,
            0, 0, 0, 0, 0, 23, 0, 71, 100, 107, 47, 87, 105, 110, 100, 111, 119, 83, 99, 97, 108,
            105, 110, 103, 70, 97, 99, 116, 111, 114, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 16, 0, 71,
            116, 107, 47, 84, 111, 111, 108, 98, 97, 114, 83, 116, 121, 108, 101, 0, 0, 0, 0, 10,
            0, 0, 0, 98, 111, 116, 104, 45, 104, 111, 114, 105, 122, 0, 0, 1, 0, 16, 0, 71, 116,
            107, 47, 75, 101, 121, 84, 104, 101, 109, 101, 78, 97, 109, 101, 0, 0, 0, 0, 7, 0, 0,
            0, 68, 101, 102, 97, 117, 108, 116, 0, 0, 0, 15, 0, 78, 101, 116, 47, 67, 117, 114,
            115, 111, 114, 66, 108, 105, 110, 107, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 18, 0, 71, 116,
            107, 47, 73, 77, 80, 114, 101, 101, 100, 105, 116, 83, 116, 121, 108, 101, 0, 0, 0, 0,
            0, 0, 8, 0, 0, 0, 99, 97, 108, 108, 98, 97, 99, 107, 0, 0, 20, 0, 71, 116, 107, 47, 69,
            110, 97, 98, 108, 101, 65, 110, 105, 109, 97, 116, 105, 111, 110, 115, 0, 0, 0, 0, 1,
            0, 0, 0, 0, 0, 7, 0, 88, 102, 116, 47, 68, 80, 73, 0, 0, 0, 0, 0, 0, 128, 1, 0, 1, 0,
            19, 0, 71, 116, 107, 47, 67, 117, 114, 115, 111, 114, 84, 104, 101, 109, 101, 78, 97,
            109, 101, 0, 0, 0, 0, 0, 7, 0, 0, 0, 65, 100, 119, 97, 105, 116, 97, 0, 1, 0, 19, 0,
            71, 116, 107, 47, 84, 111, 111, 108, 98, 97, 114, 73, 99, 111, 110, 83, 105, 122, 101,
            0, 0, 0, 0, 0, 5, 0, 0, 0, 108, 97, 114, 103, 101, 0, 0, 0, 1, 0, 17, 0, 71, 116, 107,
            47, 73, 77, 83, 116, 97, 116, 117, 115, 83, 116, 121, 108, 101, 0, 0, 0, 0, 0, 0, 0, 8,
            0, 0, 0, 99, 97, 108, 108, 98, 97, 99, 107, 0, 0, 21, 0, 71, 116, 107, 47, 82, 101, 99,
            101, 110, 116, 70, 105, 108, 101, 115, 77, 97, 120, 65, 103, 101, 0, 0, 0, 0, 0, 0, 0,
            255, 255, 255, 255, 1, 0, 11, 0, 71, 116, 107, 47, 77, 111, 100, 117, 108, 101, 115, 0,
            0, 0, 0, 0, 33, 0, 0, 0, 99, 97, 110, 98, 101, 114, 114, 97, 45, 103, 116, 107, 45,
            109, 111, 100, 117, 108, 101, 58, 112, 107, 45, 103, 116, 107, 45, 109, 111, 100, 117,
            108, 101, 0, 0, 0, 1, 0, 15, 0, 71, 116, 107, 47, 67, 111, 108, 111, 114, 83, 99, 104,
            101, 109, 101, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 17, 0, 71, 116, 107, 47, 65, 117, 116,
            111, 77, 110, 101, 109, 111, 110, 105, 99, 115, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0,
            16, 0, 71, 116, 107, 47, 77, 101, 110, 117, 66, 97, 114, 65, 99, 99, 101, 108, 0, 0, 0,
            0, 3, 0, 0, 0, 70, 49, 48, 0, 1, 0, 21, 0, 78, 101, 116, 47, 70, 97, 108, 108, 98, 97,
            99, 107, 73, 99, 111, 110, 84, 104, 101, 109, 101, 0, 0, 0, 0, 0, 0, 0, 5, 0, 0, 0,
            103, 110, 111, 109, 101, 0, 0, 0, 1, 0, 16, 0, 71, 116, 107, 47, 67, 111, 108, 111,
            114, 80, 97, 108, 101, 116, 116, 101, 0, 0, 0, 0, 148, 0, 0, 0, 98, 108, 97, 99, 107,
            58, 119, 104, 105, 116, 101, 58, 103, 114, 97, 121, 53, 48, 58, 114, 101, 100, 58, 112,
            117, 114, 112, 108, 101, 58, 98, 108, 117, 101, 58, 108, 105, 103, 104, 116, 32, 98,
            108, 117, 101, 58, 103, 114, 101, 101, 110, 58, 121, 101, 108, 108, 111, 119, 58, 111,
            114, 97, 110, 103, 101, 58, 108, 97, 118, 101, 110, 100, 101, 114, 58, 98, 114, 111,
            119, 110, 58, 103, 111, 108, 100, 101, 110, 114, 111, 100, 52, 58, 100, 111, 100, 103,
            101, 114, 32, 98, 108, 117, 101, 58, 112, 105, 110, 107, 58, 108, 105, 103, 104, 116,
            32, 103, 114, 101, 101, 110, 58, 103, 114, 97, 121, 49, 48, 58, 103, 114, 97, 121, 51,
            48, 58, 103, 114, 97, 121, 55, 53, 58, 103, 114, 97, 121, 57, 48, 0, 0, 20, 0, 71, 116,
            107, 47, 79, 118, 101, 114, 108, 97, 121, 83, 99, 114, 111, 108, 108, 105, 110, 103, 0,
            0, 0, 0, 1, 0, 0, 0, 0, 0, 21, 0, 78, 101, 116, 47, 69, 110, 97, 98, 108, 101, 69, 118,
            101, 110, 116, 83, 111, 117, 110, 100, 115, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 17,
            0, 78, 101, 116, 47, 73, 99, 111, 110, 84, 104, 101, 109, 101, 78, 97, 109, 101, 0, 0,
            0, 0, 0, 0, 0, 7, 0, 0, 0, 65, 100, 119, 97, 105, 116, 97, 0, 0, 0, 19, 0, 71, 116,
            107, 47, 83, 104, 111, 119, 85, 110, 105, 99, 111, 100, 101, 77, 101, 110, 117, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 1, 0, 16, 0, 71, 116, 107, 47, 83, 101, 115, 115, 105, 111, 110,
            66, 117, 115, 73, 100, 0, 0, 0, 0, 32, 0, 0, 0, 54, 97, 52, 98, 56, 49, 51, 102, 53,
            50, 54, 54, 99, 99, 101, 48, 54, 57, 49, 53, 57, 53, 99, 101, 57, 51, 51, 52, 54, 100,
            102, 100, 0, 0, 22, 0, 71, 116, 107, 47, 67, 117, 114, 115, 111, 114, 66, 108, 105,
            110, 107, 84, 105, 109, 101, 111, 117, 116, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0, 0, 0, 16, 0,
            71, 116, 107, 47, 66, 117, 116, 116, 111, 110, 73, 109, 97, 103, 101, 115, 0, 0, 0, 0,
            0, 0, 0, 0, 1, 0, 22, 0, 71, 116, 107, 47, 84, 105, 116, 108, 101, 98, 97, 114, 82,
            105, 103, 104, 116, 67, 108, 105, 99, 107, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 109, 101, 110,
            117, 1, 0, 18, 0, 78, 101, 116, 47, 83, 111, 117, 110, 100, 84, 104, 101, 109, 101, 78,
            97, 109, 101, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 95, 95, 99, 117, 115, 116, 111, 109, 0, 0,
            29, 0, 78, 101, 116, 47, 69, 110, 97, 98, 108, 101, 73, 110, 112, 117, 116, 70, 101,
            101, 100, 98, 97, 99, 107, 83, 111, 117, 110, 100, 115, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 1, 0, 23, 0, 71, 116, 107, 47, 84, 105, 116, 108, 101, 98, 97, 114, 68, 111, 117,
            98, 108, 101, 67, 108, 105, 99, 107, 0, 0, 0, 0, 0, 15, 0, 0, 0, 116, 111, 103, 103,
            108, 101, 45, 109, 97, 120, 105, 109, 105, 122, 101, 0, 1, 0, 8, 0, 88, 102, 116, 47,
            82, 71, 66, 65, 0, 0, 0, 0, 4, 0, 0, 0, 110, 111, 110, 101, 1, 0, 23, 0, 71, 116, 107,
            47, 84, 105, 116, 108, 101, 98, 97, 114, 77, 105, 100, 100, 108, 101, 67, 108, 105, 99,
            107, 0, 0, 0, 0, 0, 4, 0, 0, 0, 110, 111, 110, 101,
        ];

        let settings = super::parse_xsettings(&data).unwrap();
        k9::snapshot!(
            settings,
            r#"
{
    "Gdk/UnscaledDPI": Integer(
        98304,
    ),
    "Gdk/WindowScalingFactor": Integer(
        1,
    ),
    "Gtk/AutoMnemonics": Integer(
        1,
    ),
    "Gtk/ButtonImages": Integer(
        0,
    ),
    "Gtk/CanChangeAccels": Integer(
        0,
    ),
    "Gtk/ColorPalette": String(
        "black:white:gray50:red:purple:blue:light blue:green:yellow:orange:lavender:brown:goldenrod4:dodger blue:pink:light green:gray10:gray30:gray75:gray90",
    ),
    "Gtk/ColorScheme": String(
        "",
    ),
    "Gtk/CursorBlinkTimeout": Integer(
        10,
    ),
    "Gtk/CursorThemeName": String(
        "Adwaita",
    ),
    "Gtk/CursorThemeSize": Integer(
        24,
    ),
    "Gtk/DecorationLayout": String(
        "menu:close",
    ),
    "Gtk/DialogsUseHeader": Integer(
        1,
    ),
    "Gtk/EnableAnimations": Integer(
        1,
    ),
    "Gtk/EnablePrimaryPaste": Integer(
        1,
    ),
    "Gtk/FontName": String(
        "Cantarell 11",
    ),
    "Gtk/IMModule": String(
        "gtk-im-context-simple",
    ),
    "Gtk/IMPreeditStyle": String(
        "callback",
    ),
    "Gtk/IMStatusStyle": String(
        "callback",
    ),
    "Gtk/KeyThemeName": String(
        "Default",
    ),
    "Gtk/KeynavUseCaret": Integer(
        0,
    ),
    "Gtk/MenuBarAccel": String(
        "F10",
    ),
    "Gtk/MenuImages": Integer(
        0,
    ),
    "Gtk/Modules": String(
        "canberra-gtk-module:pk-gtk-module",
    ),
    "Gtk/OverlayScrolling": Integer(
        1,
    ),
    "Gtk/RecentFilesEnabled": Integer(
        1,
    ),
    "Gtk/RecentFilesMaxAge": Integer(
        -1,
    ),
    "Gtk/SessionBusId": String(
        "6a4b813f5266cce0691595ce93346dfd",
    ),
    "Gtk/ShellShowsAppMenu": Integer(
        0,
    ),
    "Gtk/ShellShowsDesktop": Integer(
        0,
    ),
    "Gtk/ShowInputMethodMenu": Integer(
        0,
    ),
    "Gtk/ShowUnicodeMenu": Integer(
        0,
    ),
    "Gtk/TimeoutInitial": Integer(
        200,
    ),
    "Gtk/TimeoutRepeat": Integer(
        20,
    ),
    "Gtk/TitlebarDoubleClick": String(
        "toggle-maximize",
    ),
    "Gtk/TitlebarMiddleClick": String(
        "none",
    ),
    "Gtk/TitlebarRightClick": String(
        "menu",
    ),
    "Gtk/ToolbarIconSize": String(
        "large",
    ),
    "Gtk/ToolbarStyle": String(
        "both-horiz",
    ),
    "Net/CursorBlink": Integer(
        1,
    ),
    "Net/CursorBlinkTime": Integer(
        1200,
    ),
    "Net/DndDragThreshold": Integer(
        8,
    ),
    "Net/DoubleClickTime": Integer(
        400,
    ),
    "Net/EnableEventSounds": Integer(
        1,
    ),
    "Net/EnableInputFeedbackSounds": Integer(
        0,
    ),
    "Net/FallbackIconTheme": String(
        "gnome",
    ),
    "Net/IconThemeName": String(
        "Adwaita",
    ),
    "Net/SoundThemeName": String(
        "__custom",
    ),
    "Net/ThemeName": String(
        "Adwaita-dark",
    ),
    "Xft/Antialias": Integer(
        1,
    ),
    "Xft/DPI": Integer(
        98304,
    ),
    "Xft/HintStyle": String(
        "hintslight",
    ),
    "Xft/Hinting": Integer(
        1,
    ),
    "Xft/RGBA": String(
        "none",
    ),
}
"#
        );
    }
}
