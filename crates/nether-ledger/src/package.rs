//! Cairn-addressed package storage and offline resolution (`spec/07-ledger.md` §7.5.5).

use std::collections::{BTreeMap, BTreeSet};

use crate::cairn::Cairn;
use crate::codec::{DecodeError, Stored};
use crate::store::{MAX_OBJECT_BYTES, Store, StoreError};
use crate::value::{NonEmptyName, Value};

/// The domain version string a current package declares (`spec/07-ledger.md` §7.5.5).
pub const PACKAGE_DOMAIN: &str = "netherc/cairn/v4";

/// Why a package could not be written or resolved offline (`spec/07-ledger.md` §7.5.5).
#[derive(Debug)]
pub enum PackageError {
    /// A package or entrypoint object was absent or corrupt in the store.
    Store(StoreError),
    /// A package `(cairn, domain)` declared a domain version other than [`PACKAGE_DOMAIN`].
    Incompatible(Cairn, String),
    /// A package or entrypoint `(cairn, why)` violated §7.5.5's structural rules.
    Malformed(Cairn, &'static str),
}

impl core::fmt::Display for PackageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "{e}"),
            Self::Incompatible(c, domain) => {
                write!(f, "package {} uses incompatible domain `{domain}`", c.short())
            }
            Self::Malformed(c, why) => write!(f, "malformed package {}: {why}", c.short()),
        }
    }
}

impl core::error::Error for PackageError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Store(e) => Some(e),
            Self::Incompatible(..) | Self::Malformed(..) => None,
        }
    }
}

impl From<StoreError> for PackageError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

/// An offline-resolved package closure (`spec/07-ledger.md` §7.5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackage {
    /// The root package cairn.
    pub root: Cairn,
    /// The root package's `entrypoint` `Bytes` cairn.
    pub entrypoint: Cairn,
    /// Direct alias-to-package-cairn bindings declared on `root`.
    pub aliases: BTreeMap<String, Cairn>,
    /// Distinct package cairns in topological post-order (dependencies first, `root` last).
    pub closure: Vec<Cairn>,
    /// Concatenated UTF-8 `entrypoint` bytes in `closure` order.
    pub unit_source: Vec<u8>,
    /// Content address of `Value::Bytes(unit_source)`.
    pub unit_cairn: Cairn,
}

impl ResolvedPackage {
    /// Resolves a direct alias declared on `root` to its exact package cairn.
    #[must_use]
    pub fn alias(&self, name: &str) -> Option<Cairn> {
        self.aliases.get(name).copied()
    }
}

/// Cairn-addressed package storage and offline dependency resolution (`spec/07-ledger.md` §7.5.5).
pub struct Package;

/// The decoded entrypoint cairn and sorted dependency list of a single
/// `Package` record. Entrypoint bytes are loaded lazily in post-order during
/// [`Package::walk`] so ancestor sources are never pinned on the DFS stack.
type LoadedPkg = (Cairn, Vec<(String, Cairn)>);

impl Package {
    /// Writes `entrypoint_source` and a canonical `"Package"` struct with
    /// sorted `deps` `(alias, package_cairn)` into `store`, returning the
    /// package cairn.
    ///
    /// # Errors
    ///
    /// Returns [`PackageError::Malformed`] if `entrypoint_source` is not UTF-8
    /// or if `deps` has an empty or duplicate alias, or [`PackageError::Store`]
    /// if writing to `store` fails.
    pub fn put(
        store: &Store,
        entrypoint_source: &[u8],
        deps: &[(&str, Cairn)],
    ) -> Result<Cairn, PackageError> {
        let ep_val = Value::Bytes(entrypoint_source.to_vec());
        if core::str::from_utf8(entrypoint_source).is_err() {
            return Err(PackageError::Malformed(ep_val.cairn(), "entrypoint is not UTF-8"));
        }
        let mut sorted = BTreeMap::new();
        for &(alias, dep) in deps {
            if alias.is_empty() || sorted.insert(alias, dep).is_some() {
                return Err(PackageError::Malformed(ep_val.cairn(), "invalid or duplicate alias"));
            }
        }
        let entrypoint = store.put(&Stored::Value(ep_val))?;
        let dep_name = NonEmptyName::new("Dep")
            .map_err(|_| StoreError::Invalid(DecodeError::EmptyStructName))?;
        let pkg_name = NonEmptyName::new("Package")
            .map_err(|_| StoreError::Invalid(DecodeError::EmptyStructName))?;
        let dep_items = sorted
            .into_iter()
            .map(|(alias, cairn)| Value::Struct {
                name: dep_name.clone(),
                fields: vec![Value::Str(alias.into()), Value::Cairn(cairn)],
            })
            .collect();
        let pkg = Value::Struct {
            name: pkg_name,
            fields: vec![
                Value::Str(PACKAGE_DOMAIN.into()),
                Value::Cairn(entrypoint),
                Value::Array(dep_items),
            ],
        };
        Ok(store.put(&Stored::Value(pkg))?)
    }

    /// Resolves `root` and its transitive dependency closure offline in `store`.
    ///
    /// # Errors
    ///
    /// Returns [`PackageError`] if any reachable package or entrypoint object
    /// is absent, corrupt, incompatible or malformed.
    pub fn resolve(store: &Store, root: Cairn) -> Result<ResolvedPackage, PackageError> {
        let (mut visiting, mut visited) = (BTreeSet::new(), BTreeSet::new());
        let (mut closure, mut unit_source) = (Vec::new(), Vec::new());
        let (entrypoint, root_deps) =
            Self::walk(store, root, &mut visiting, &mut visited, &mut closure, &mut unit_source)?;
        let unit_cairn = Cairn::of_bytes_value(&unit_source);
        Ok(ResolvedPackage {
            root,
            entrypoint,
            aliases: root_deps.into_iter().collect(),
            closure,
            unit_source,
            unit_cairn,
        })
    }

    fn load_pkg(store: &Store, pkg: Cairn) -> Result<LoadedPkg, PackageError> {
        let Stored::Value(Value::Struct { name, fields }) = store.get(pkg)? else {
            return Err(PackageError::Malformed(pkg, "not a Package struct"));
        };
        if name != "Package" {
            return Err(PackageError::Malformed(pkg, "not a Package struct"));
        }
        if let Some(Value::Str(domain)) = fields.first()
            && domain != PACKAGE_DOMAIN
        {
            return Err(PackageError::Incompatible(pkg, domain.clone()));
        }
        let [Value::Str(_), Value::Cairn(ep), Value::Array(raw_deps)] = &fields[..] else {
            return Err(PackageError::Malformed(pkg, "wrong Package fields"));
        };
        let mut deps = Vec::with_capacity(raw_deps.len());
        let mut prev = "";
        for item in raw_deps {
            let Value::Struct { name: dname, fields: dfields } = item else {
                return Err(PackageError::Malformed(pkg, "not a Dep struct"));
            };
            let [Value::Str(alias), Value::Cairn(target)] = &dfields[..] else {
                return Err(PackageError::Malformed(pkg, "wrong Dep fields"));
            };
            if dname != "Dep" || alias.is_empty() || alias.as_str() <= prev || *target == pkg {
                return Err(PackageError::Malformed(pkg, "invalid or unsorted Dep"));
            }
            prev = alias;
            deps.push((alias.clone(), *target));
        }
        Ok((*ep, deps))
    }

    fn load_entrypoint(store: &Store, ep: Cairn) -> Result<Vec<u8>, PackageError> {
        let Stored::Value(Value::Bytes(bytes)) = store.get(ep)? else {
            return Err(PackageError::Malformed(ep, "entrypoint is not Bytes"));
        };
        if core::str::from_utf8(&bytes).is_err() {
            return Err(PackageError::Malformed(ep, "entrypoint is not UTF-8"));
        }
        Ok(bytes)
    }

    fn walk(
        store: &Store,
        root: Cairn,
        visiting: &mut BTreeSet<Cairn>,
        visited: &mut BTreeSet<Cairn>,
        closure: &mut Vec<Cairn>,
        unit_source: &mut Vec<u8>,
    ) -> Result<(Cairn, Vec<(String, Cairn)>), PackageError> {
        struct Frame {
            pkg: Cairn,
            ep: Cairn,
            deps: Vec<(String, Cairn)>,
            next_dep: usize,
        }
        let (root_ep, root_deps) = Self::load_pkg(store, root)?;
        visiting.insert(root);
        let mut stack =
            vec![Frame { pkg: root, ep: root_ep, deps: root_deps.clone(), next_dep: 0 }];
        while let Some(top) = stack.last_mut() {
            if top.next_dep < top.deps.len() {
                let dep = top.deps[top.next_dep].1;
                top.next_dep += 1;
                if visited.contains(&dep) {
                    continue;
                }
                if !visiting.insert(dep) {
                    return Err(PackageError::Malformed(dep, "dependency cycle"));
                }
                let (ep, deps) = Self::load_pkg(store, dep)?;
                stack.push(Frame { pkg: dep, ep, deps, next_dep: 0 });
            } else {
                let Some(done) = stack.pop() else { break };
                visiting.remove(&done.pkg);
                if visited.insert(done.pkg) {
                    let bytes = Self::load_entrypoint(store, done.ep)?;
                    let needs_nl = !bytes.is_empty()
                        && !unit_source.is_empty()
                        && !unit_source.ends_with(b"\n");
                    let next_len = unit_source
                        .len()
                        .saturating_add(usize::from(needs_nl))
                        .saturating_add(bytes.len());
                    if next_len > MAX_OBJECT_BYTES {
                        return Err(PackageError::Malformed(
                            done.pkg,
                            "resolved package source exceeds size bound",
                        ));
                    }
                    closure.push(done.pkg);
                    if needs_nl {
                        unit_source.push(b'\n');
                    }
                    unit_source.extend_from_slice(&bytes);
                }
            }
        }
        Ok((root_ep, root_deps))
    }
}
