//! Interact with netcdf dimensions

#![allow(clippy::similar_names)]
use super::error;
use netcdf_sys::*;
use std::convert::TryInto;
use std::marker::PhantomData;

/// Represents a netcdf dimension
#[derive(Debug, Clone)]
pub struct Dimension<'g> {
    /// None when unlimited (size = 0)
    pub(crate) len: Option<core::num::NonZeroUsize>,
    pub(crate) id: Identifier,
    pub(crate) _group: PhantomData<&'g nc_type>,
}

/// Unique identifier for a dimensions in a file. Used when
/// names can not be used directly
#[derive(Debug, Copy, Clone)]
pub struct Identifier {
    pub(crate) ncid: nc_type,
    pub(crate) dimid: nc_type,
}

#[allow(clippy::len_without_is_empty)]
impl<'g> Dimension<'g> {
    /// Get current length of this dimension
    pub fn len(&self) -> usize {
        if let Some(x) = self.len {
            x.get()
        } else {
            let mut len = 0;
            let err = unsafe {
                // Must lock in case other variables adds to the dimension length
                error::checked(super::with_lock(|| {
                    nc_inq_dimlen(self.id.ncid, self.id.dimid, &mut len)
                }))
            };

            // Should log or handle this somehow...
            err.map(|_| len).unwrap_or(0)
        }
    }

    /// Checks whether the dimension is growable
    pub fn is_unlimited(&self) -> bool {
        self.len.is_none()
    }

    /// Gets the name of the dimension
    pub fn name(&self) -> String {
        let mut name = vec![0_u8; NC_MAX_NAME as usize + 1];
        unsafe {
            error::checked(super::with_lock(|| {
                nc_inq_dimname(self.id.ncid, self.id.dimid, name.as_mut_ptr() as *mut _)
            }))
            .unwrap();
        }

        let zeropos = name
            .iter()
            .position(|&x| x == 0)
            .unwrap_or_else(|| name.len());
        name.resize(zeropos, 0);
        String::from_utf8(name).expect("Dimension did not have a valid name")
    }

    /// Grabs the unique identifier for this dimension, which
    /// can be used in `add_variable_from_identifiers`
    pub fn identifier(&self) -> Identifier {
        self.id
    }
}

pub(crate) fn from_name_toid(loc: nc_type, name: &str) -> error::Result<Option<nc_type>> {
    let mut dimid = 0;
    let cname = super::utils::short_name_to_bytes(name)?;
    let e =
        unsafe { super::with_lock(|| nc_inq_dimid(loc, cname.as_ptr() as *const _, &mut dimid)) };
    if e == NC_EBADDIM {
        return Ok(None);
    } else {
        error::checked(e)?;
    }
    Ok(Some(dimid))
}

pub(crate) fn from_name<'f>(loc: nc_type, name: &str) -> error::Result<Option<Dimension<'f>>> {
    let mut dimid = 0;
    let cname = super::utils::short_name_to_bytes(name)?;
    let e =
        unsafe { super::with_lock(|| nc_inq_dimid(loc, cname.as_ptr() as *const _, &mut dimid)) };
    if e == NC_EBADDIM {
        return Ok(None);
    } else {
        error::checked(e)?;
    }
    let mut dimlen = 0;
    unsafe {
        error::checked(super::with_lock(|| nc_inq_dimlen(loc, dimid, &mut dimlen)))?;
    }

    Ok(Some(Dimension {
        len: core::num::NonZeroUsize::new(dimlen),
        id: Identifier { ncid: loc, dimid },
        _group: PhantomData,
    }))
}

pub(crate) fn dimensions_from_location<'g>(
    ncid: nc_type,
) -> error::Result<impl Iterator<Item = error::Result<Dimension<'g>>>> {
    let mut ndims = 0;
    unsafe {
        error::checked(super::with_lock(|| {
            nc_inq_dimids(ncid, &mut ndims, std::ptr::null_mut(), false as _)
        }))?;
    }
    let mut dimids = vec![0; ndims.try_into()?];
    unsafe {
        error::checked(super::with_lock(|| {
            nc_inq_dimids(ncid, std::ptr::null_mut(), dimids.as_mut_ptr(), false as _)
        }))?;
    }
    Ok(dimids.into_iter().map(move |dimid| {
        let mut dimlen = 0;
        unsafe {
            error::checked(super::with_lock(|| nc_inq_dimlen(ncid, dimid, &mut dimlen)))?;
        }
        Ok(Dimension {
            len: core::num::NonZeroUsize::new(dimlen),
            id: Identifier { ncid, dimid },
            _group: PhantomData,
        })
    }))
}

pub(crate) fn dimensions_from_variable<'g>(
    ncid: nc_type,
    varid: nc_type,
) -> error::Result<impl Iterator<Item = error::Result<Dimension<'g>>>> {
    let mut ndims = 0;
    unsafe {
        error::checked(super::with_lock(|| {
            nc_inq_varndims(ncid, varid, &mut ndims)
        }))?;
    }
    let mut dimids = vec![0; ndims.try_into()?];
    unsafe {
        error::checked(super::with_lock(|| {
            nc_inq_vardimid(ncid, varid, dimids.as_mut_ptr())
        }))?;
    }

    Ok(dimids.into_iter().map(move |dimid| {
        let mut dimlen = 0;
        unsafe {
            error::checked(super::with_lock(|| nc_inq_dimlen(ncid, dimid, &mut dimlen)))?;
        }
        Ok(Dimension {
            len: core::num::NonZeroUsize::new(dimlen),
            id: Identifier { ncid, dimid },
            _group: PhantomData,
        })
    }))
}

pub(crate) fn dimension_from_name<'f>(
    ncid: nc_type,
    name: &str,
) -> error::Result<Option<Dimension<'f>>> {
    let cname = super::utils::short_name_to_bytes(name)?;
    let mut dimid = 0;
    let e =
        unsafe { super::with_lock(|| nc_inq_dimid(ncid, cname.as_ptr() as *const _, &mut dimid)) };
    if e == NC_EBADDIM {
        return Ok(None);
    } else {
        error::checked(e)?;
    }
    let mut dimlen = 0;
    unsafe {
        error::checked(super::with_lock(|| nc_inq_dimlen(ncid, dimid, &mut dimlen))).unwrap();
    }
    Ok(Some(Dimension {
        len: core::num::NonZeroUsize::new(dimlen),
        id: super::dimension::Identifier { ncid, dimid },
        _group: PhantomData,
    }))
}

pub(crate) fn add_dimension_at<'f>(
    ncid: nc_type,
    name: &str,
    len: usize,
) -> error::Result<Dimension<'f>> {
    let cname = super::utils::short_name_to_bytes(name)?;
    let mut dimid = 0;
    unsafe {
        error::checked(super::with_lock(|| {
            nc_def_dim(ncid, cname.as_ptr() as *const _, len, &mut dimid)
        }))?;
    }
    Ok(Dimension {
        len: core::num::NonZeroUsize::new(dimid.try_into()?),
        id: Identifier { ncid, dimid },
        _group: PhantomData,
    })
}
