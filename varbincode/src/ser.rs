use crate::error::Error;
use byteorder::{LittleEndian, WriteBytesExt};
use serde::ser;

pub struct Serializer<'a> {
    writer: &'a mut std::io::Write,
}

impl<'a> Serializer<'a> {
    pub fn new(writer: &'a mut std::io::Write) -> Self {
        Self { writer }
    }

    fn write_signed(&mut self, val: i64) -> Result<usize, std::io::Error> {
        leb128::write::signed(&mut self.writer, val)
    }

    fn write_unsigned(&mut self, val: u64) -> Result<usize, std::io::Error> {
        leb128::write::unsigned(&mut self.writer, val)
    }
}

impl<'a, 'b> ser::Serializer for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<(), Error> {
        self.write_unsigned(if v { 1 } else { 0 })?;
        Ok(())
    }

    fn serialize_unit(self) -> Result<(), Error> {
        Ok(())
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Error> {
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<(), Error> {
        self.writer.write_u8(v as _)?;
        Ok(())
    }

    fn serialize_u16(self, v: u16) -> Result<(), Error> {
        self.write_unsigned(v as _)?;
        Ok(())
    }

    fn serialize_u32(self, v: u32) -> Result<(), Error> {
        self.write_unsigned(v as _)?;
        Ok(())
    }

    fn serialize_u64(self, v: u64) -> Result<(), Error> {
        self.write_unsigned(v as _)?;
        Ok(())
    }

    fn serialize_i8(self, v: i8) -> Result<(), Error> {
        self.writer.write_i8(v as _)?;
        Ok(())
    }

    fn serialize_i16(self, v: i16) -> Result<(), Error> {
        self.write_signed(v as _)?;
        Ok(())
    }

    fn serialize_i32(self, v: i32) -> Result<(), Error> {
        self.write_signed(v as _)?;
        Ok(())
    }

    fn serialize_i64(self, v: i64) -> Result<(), Error> {
        self.write_signed(v as _)?;
        Ok(())
    }

    fn serialize_f32(self, v: f32) -> Result<(), Error> {
        self.writer.write_f32::<LittleEndian>(v)?;
        Ok(())
    }

    fn serialize_f64(self, v: f64) -> Result<(), Error> {
        self.writer.write_f64::<LittleEndian>(v)?;
        Ok(())
    }

    fn serialize_str(self, v: &str) -> Result<(), Error> {
        self.serialize_bytes(v.as_bytes())
    }

    fn serialize_char(self, c: char) -> Result<(), Error> {
        self.serialize_u32(c as u32)
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<(), Error> {
        self.serialize_u64(v.len() as u64)?;
        self.writer.write_all(v)?;
        Ok(())
    }

    fn serialize_none(self) -> Result<(), Error> {
        self.serialize_u8(0)
    }

    fn serialize_some<T: serde::Serialize + ?Sized>(self, v: &T) -> Result<(), Error> {
        self.serialize_u8(1)?;
        v.serialize(self)
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
        let len = len.ok_or(Error::SequenceMustHaveLength)?;
        self.serialize_u64(len as u64)?;
        Ok(self)
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
        Ok(self)
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Error> {
        Ok(self)
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Error> {
        self.serialize_u32(variant_index)?;
        Ok(self)
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Error> {
        let len = len.ok_or(Error::SequenceMustHaveLength)?;
        self.serialize_u64(len as u64)?;
        Ok(self)
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Error> {
        Ok(self)
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Error> {
        self.serialize_u32(variant_index)?;
        Ok(self)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), Error>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<(), Error>
    where
        T: serde::ser::Serialize,
    {
        self.serialize_u32(variant_index)?;
        value.serialize(self)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        _variant: &'static str,
    ) -> Result<(), Error> {
        self.serialize_u32(variant_index)
    }

    fn is_human_readable(&self) -> bool {
        false
    }
}

impl<'a, 'b> ser::SerializeSeq for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, 'b> ser::SerializeTuple for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, 'b> ser::SerializeTupleStruct for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, 'b> ser::SerializeTupleVariant for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, 'b> ser::SerializeMap for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_key<K: ?Sized>(&mut self, value: &K) -> Result<(), Error>
    where
        K: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn serialize_value<V: ?Sized>(&mut self, value: &V) -> Result<(), Error>
    where
        V: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, 'b> ser::SerializeStruct for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, _key: &'static str, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}

impl<'a, 'b> ser::SerializeStructVariant for &'a mut Serializer<'b> {
    type Ok = ();
    type Error = Error;

    #[inline]
    fn serialize_field<T: ?Sized>(&mut self, _key: &'static str, value: &T) -> Result<(), Error>
    where
        T: serde::ser::Serialize,
    {
        value.serialize(&mut **self)
    }

    #[inline]
    fn end(self) -> Result<(), Error> {
        Ok(())
    }
}
