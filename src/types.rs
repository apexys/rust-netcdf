//! `NetCDF` types for variables. Both inbuilt and user types are found here

use netcdf_sys::*;
use super::*;

/// A `netCDF` type
pub enum Type {
    /// The atomic type
    Simple(SimpleType),
    /// A string type
    String,
}

/// Simple atomic types
#[allow(missing_docs)]
pub enum SimpleType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

pub(crate) fn type_from_name(ncid: nc_type, name: &str) -> error::Result<Option<Type>> {
    let byte_name = utils::short_name_to_bytes(name)?;
    let mut xtype = 0;
    let e = unsafe {
        nc_inq_typeid(ncid, byte_name.as_ptr() as *const _, &mut xtype)
    };
    if e == NC_EBADTYPE {
        return Ok(None);
    } else {
        error::checked(e)?;
    }
    match xtype {
        NC_UBYTE => return Ok(Some(Type::Simple(SimpleType::U8))),
        NC_BYTE => return Ok(Some(Type::Simple(SimpleType::I8))),
        NC_USHORT => return Ok(Some(Type::Simple(SimpleType::U16))),
        NC_SHORT => return Ok(Some(Type::Simple(SimpleType::I16))),
        NC_UINT => return Ok(Some(Type::Simple(SimpleType::U32))),
        NC_INT => return Ok(Some(Type::Simple(SimpleType::I32))),
        NC_UINT64 => return Ok(Some(Type::Simple(SimpleType::U64))),
        NC_INT64 => return Ok(Some(Type::Simple(SimpleType::I64))),
        NC_FLOAT => return Ok(Some(Type::Simple(SimpleType::F32))),
        NC_DOUBLE => return Ok(Some(Type::Simple(SimpleType::F64))),
        NC_STRING => return Ok(Some(Type::String)),
        _ => (),
    }
    todo!("User defined types")
}

pub(crate) fn is_simple_ncid(ncid: ncid, varid: nc_type) -> error::Result<bool> {
    let mut xtype = 0;
    unsafe {
        error::checked(nc_inq_vartype(ncid, varid, &mut xtype))?;
    }
    Ok(is_simple(xtype))
}

fn is_simple(xtype: nc_type) -> bool {
    match xtype {
        NC_UBYTE | 
        NC_BYTE |
        NC_USHORT |
        NC_SHORT |
        NC_UINT |
        NC_INT |
        NC_UINT64 |
        NC_INT64 | 
        NC_FLOAT |
        NC_DOUBLE => true,
        _ => false,
    }
}
