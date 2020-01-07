//! All netcdf items belong in the root group, which can
//! be interacted with to get the underlying data

use super::attribute::AttrValue;
use super::attribute::Attribute;
use super::dimension::Dimension;
use super::error;
use super::variable::{Numeric, Variable, VariableMut};
use netcdf_sys::*;
use std::convert::TryInto;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

/// Main component of the netcdf format. Holds all variables,
/// attributes, and dimensions. A group can always see the parents items,
/// but a parent can not access a childs items.
#[derive(Debug, Clone)]
pub struct Group<'f> {
    pub(crate) ncid: nc_type,
    pub(crate) lock: Arc<Mutex<()>>,
    pub(crate) _file: PhantomData<&'f nc_type>,
}

#[derive(Debug)]
/// Mutable access to a group
#[allow(clippy::module_name_repetitions)]
pub struct GroupMut<'f>(
    pub(crate) Group<'f>,
    pub(crate) PhantomData<&'f mut nc_type>,
);

impl<'f> std::ops::Deref for GroupMut<'f> {
    type Target = Group<'f>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'f> Group<'f> {
    /// Name of the current group
    pub fn name(&self) -> error::Result<String> {
        let mut name = vec![0_u8; NC_MAX_NAME as usize + 1];
        unsafe {
            error::checked(nc_inq_grpname(self.ncid, name.as_mut_ptr() as *mut _))?;
        }
        let zeropos = name
            .iter()
            .position(|&x| x == 0)
            .unwrap_or_else(|| name.len());
        name.resize(zeropos, 0);

        Ok(String::from_utf8(name)?)
    }
    /// Internal ncid of the group
    fn id(&self) -> nc_type {
        self.ncid
    }

    /// Get a variable from the group
    pub fn variable<'g>(&'g self, name: &str) -> error::Result<Option<Variable<'g>>>
    where
        'f: 'g,
    {
        Variable::find_from_name(self.id(), name, self.lock.clone())
    }
    /// Iterate over all variables in a group
    pub fn variables<'g>(
        &'g self,
    ) -> error::Result<impl Iterator<Item = error::Result<Variable<'g>>>>
    where
        'f: 'g,
    {
        super::variable::variables_at_ncid(self.id(), self.lock.clone())
    }

    /// Get a single attribute
    pub fn attribute<'a>(&'a self, name: &str) -> error::Result<Option<Attribute<'a>>> {
        let _l = self.lock.lock().unwrap();
        Attribute::find_from_name(self.ncid, None, name)
    }
    /// Get all attributes in the group
    pub fn attributes<'a>(
        &'a self,
    ) -> error::Result<impl Iterator<Item = error::Result<Attribute<'a>>>> {
        // Need to lock when reading the first attribute (per group)
        let _l = self.lock.lock().unwrap();
        crate::attribute::AttributeIterator::new(self.ncid, None)
    }

    /// Get a single dimension
    pub fn dimension<'g>(&'g self, name: &str) -> error::Result<Option<Dimension<'g>>>
    where
        'f: 'g,
    {
        super::dimension::dimension_from_name(self.id(), name)
    }
    /// Iterator over all dimensions
    pub fn dimensions<'g>(
        &'g self,
    ) -> error::Result<impl Iterator<Item = error::Result<Dimension<'g>>>>
    where
        'f: 'g,
    {
        super::dimension::dimensions_from_location(self.id())
    }

    /// Get a group
    pub fn group<'g>(&'g self, name: &str) -> error::Result<Option<Group<'g>>>
    where
        'f: 'g,
    {
        group_from_name(self.id(), name)
    }
    /// Iterator over all subgroups in this group
    pub fn groups<'g>(&'g self) -> error::Result<impl Iterator<Item = Group<'g>>>
    where
        'f: 'g,
    {
        groups_at_ncid(self.id(), self.lock.clone())
    }
}

impl<'f> GroupMut<'f> {
    /// Get a mutable variable from the group
    pub fn variable_mut<'g>(&'g mut self, name: &str) -> error::Result<Option<VariableMut<'g>>>
    where
        'f: 'g,
    {
        self.variable(name)
            .map(|v| v.map(|v| VariableMut(v, PhantomData)))
    }
    /// Iterate over all variables in a group, with mutable access
    pub fn variables_mut<'g>(
        &'g mut self,
    ) -> error::Result<impl Iterator<Item = error::Result<VariableMut<'g>>>>
    where
        'f: 'g,
    {
        self.variables()
            .map(|var| var.map(|var| var.map(|var| VariableMut(var, PhantomData))))
    }

    /// Mutable access to subgroup
    pub fn group_mut<'g>(&'g mut self, name: &str) -> error::Result<Option<GroupMut<'g>>>
    where
        'f: 'g,
    {
        self.group(name)
            .map(|g| g.map(|g| GroupMut(g, PhantomData)))
    }
    /// Iterator over all groups (mutable access)
    pub fn groups_mut<'g>(&'g mut self) -> error::Result<impl Iterator<Item = GroupMut<'g>>>
    where
        'f: 'g,
    {
        self.groups().map(|g| g.map(|g| GroupMut(g, PhantomData)))
    }

    /// Add an attribute to the group
    pub fn add_attribute<'a, T>(&'a mut self, name: &str, val: T) -> error::Result<Attribute<'a>>
    where
        T: Into<AttrValue>,
    {
        let _l = self.lock.lock().unwrap();
        Attribute::put(self.ncid, NC_GLOBAL, name, val.into())
    }

    /// Adds a dimension with the given name and size. A size of zero gives an unlimited dimension
    pub fn add_dimension<'g>(&'g mut self, name: &str, len: usize) -> error::Result<Dimension<'g>> {
        let _l = self.lock.lock().unwrap();
        super::dimension::add_dimension_at(self.id(), name, len)
    }

    /// Adds a dimension with unbounded size
    pub fn add_unlimited_dimension<'g>(&'g mut self, name: &str) -> error::Result<Dimension<'g>> {
        self.add_dimension(name, 0)
    }

    pub(crate) fn add_group_at(ncid: nc_type, name: &str) -> error::Result<Self> {
        let byte_name = super::utils::short_name_to_bytes(name)?;
        let mut grpid = 0;
        unsafe {
            error::checked(nc_def_grp(ncid, byte_name.as_ptr() as *const _, &mut grpid))?;
        }

        Ok(Self(
            Group {
                ncid: grpid,
                lock: Arc::new(Mutex::new(())),
                _file: PhantomData,
            },
            PhantomData,
        ))
    }

    /// Add an empty group to the dataset
    pub fn add_group<'g>(&'g mut self, name: &str) -> error::Result<GroupMut<'g>>
    where
        'f: 'g,
    {
        let _l = self.0.lock.lock().unwrap();
        Self::add_group_at(self.id(), name)
    }

    /// Create a Variable into the dataset, with no data written into it
    ///
    /// Dimensions are identified using the name of the dimension, and will recurse upwards
    /// if not found in the current group.
    pub fn add_variable<'g, T>(
        &'g mut self,
        name: &str,
        dims: &[&str],
    ) -> error::Result<VariableMut<'g>>
    where
        T: Numeric,
        'f: 'g,
    {
        let _l = self.0.lock.lock().unwrap();
        VariableMut::add_from_str(self.id(), T::NCTYPE, name, dims, self.0.lock.clone())
    }
    /// Adds a variable with a basic type of string
    pub fn add_string_variable<'g>(
        &mut self,
        name: &str,
        dims: &[&str],
    ) -> error::Result<VariableMut<'g>> {
        let _l = self.0.lock.lock().unwrap();
        VariableMut::add_from_str(self.id(), NC_STRING, name, dims, self.0.lock.clone())
    }
    /// Adds a variable from a set of unique identifiers, recursing upwards
    /// from the current group if necessary.
    pub fn add_variable_from_identifiers<'g, T>(
        &'g mut self,
        name: &str,
        dims: &[super::dimension::Identifier],
    ) -> error::Result<VariableMut<'g>>
    where
        T: Numeric,
    {
        let _l = self.0.lock.lock().unwrap();
        super::variable::add_variable_from_identifiers(
            self.id(),
            name,
            dims,
            T::NCTYPE,
            self.0.lock.clone(),
        )
    }
}

pub(crate) fn groups_at_ncid<'f>(
    ncid: nc_type,
    lock: Arc<Mutex<()>>,
) -> error::Result<impl Iterator<Item = Group<'f>>> {
    let mut num_grps = 0;
    unsafe {
        error::checked(nc_inq_grps(ncid, &mut num_grps, std::ptr::null_mut()))?;
    }
    let mut grps = vec![0; num_grps.try_into()?];
    unsafe {
        error::checked(nc_inq_grps(ncid, std::ptr::null_mut(), grps.as_mut_ptr()))?;
    }
    Ok(grps.into_iter().map(move |id| Group {
        ncid: id,
        lock: lock.clone(),
        _file: PhantomData,
    }))
}

pub(crate) fn group_from_name<'f>(ncid: nc_type, name: &str) -> error::Result<Option<Group<'f>>> {
    let byte_name = super::utils::short_name_to_bytes(name)?;
    let mut grpid = 0;
    let e = unsafe { nc_inq_grp_ncid(ncid, byte_name.as_ptr() as *const _, &mut grpid) };
    if e == NC_ENOGRP {
        return Ok(None);
    } else {
        error::checked(e)?;
    }
    Ok(Some(Group {
        ncid: grpid,
        lock: Arc::new(Mutex::new(())),
        _file: PhantomData,
    }))
}
