use std::collections::HashMap;

/// Parses:
/// $ xprop -root | grep RESOURCE_MANAGER
/// RESOURCE_MANAGER(STRING) = "Xft.dpi:\t96\nXft.hinting:\t1\nXft.hintstyle:\thintslight\nXft.antialias:\t1\nXft.rgba:\tnone\nXcursor.size:\t24\nXcursor.theme:\tAdwaita\n"
pub fn parse_root_resource_manager(
    conn: &xcb::Connection,
    root: xcb::xproto::Window,
) -> anyhow::Result<HashMap<String, String>> {
    let reply = xcb::xproto::get_property(
        conn,
        false,
        root,
        xcb::ffi::XCB_ATOM_RESOURCE_MANAGER,
        xcb::xproto::ATOM_STRING,
        0,
        1024 * 1024,
    )
    .get_reply()?;

    let text = String::from_utf8_lossy(reply.value::<u8>());
    let mut map = HashMap::new();
    for line in text.split('\n') {
        if let Some(colon) = line.find(':') {
            let (key, value) = line.split_at(colon);
            let key = key.trim();
            let value = value[1..].trim();

            map.insert(key.to_string(), value.to_string());
        }
    }

    Ok(map)
}
