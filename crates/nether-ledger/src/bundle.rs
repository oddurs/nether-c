//! Portable store bundles (`spec/07-ledger.md` §7.5.4).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};
use std::path::PathBuf;

use crate::cairn::Cairn;
use crate::codec::scan_stored;
use crate::store::{MAX_OBJECT_BYTES, Store, StoreError, TempFileGuard, read_bounded, verify_with};

pub(crate) const BUNDLE_MAGIC: &[u8] = b"netherc/cairn/v4\x00bundle\x00";
const MAX_BUNDLE_OBJECTS: usize = 1_000_000;
const MAX_BUNDLE_BYTES: usize = 512 * 1024 * 1024;

fn read_bundle_exact(src: &mut impl io::Read, buf: &mut [u8]) -> Result<(), StoreError> {
    src.read_exact(buf).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            StoreError::Bundle("truncated bundle")
        } else {
            StoreError::Io(e)
        }
    })
}

fn read_hashed(
    src: &mut impl io::Read,
    hasher: &mut blake3::Hasher,
    buf: &mut [u8],
) -> Result<(), StoreError> {
    read_bundle_exact(src, buf)?;
    hasher.update(buf);
    Ok(())
}

/// An object's height is 0 if it references nothing here, and one more than
/// the highest thing it references otherwise. Sorting by height and then by
/// cairn is §7.5.4's order: everything an object references comes before it.
/// Walked with a stack, because a chain of traces is as deep as the store is long.
fn object_heights(
    found: &BTreeMap<Cairn, (Vec<Cairn>, PathBuf)>,
) -> Result<BTreeMap<Cairn, usize>, StoreError> {
    let mut height: BTreeMap<Cairn, usize> = BTreeMap::new();
    let mut open = BTreeSet::new();
    for &start in found.keys() {
        let mut todo = vec![start];
        while let Some(&c) = todo.last() {
            if height.contains_key(&c) {
                todo.pop();
                continue;
            }
            let refs = found[&c].0.iter().filter(|r| found.contains_key(r));
            if open.insert(c) {
                for &r in refs {
                    // §7.3: a cairn names its references, so a cycle would
                    // be a hash that contains itself.
                    if open.contains(&r) {
                        return Err(StoreError::Bundle("a cycle in objects/"));
                    }
                    if !height.contains_key(&r) {
                        todo.push(r);
                    }
                }
                continue;
            }
            let h = refs.map(|r| height[r] + 1).max().unwrap_or(0);
            height.insert(c, h);
            open.remove(&c);
            todo.pop();
        }
    }
    Ok(height)
}

impl Store {
    /// Exports every verified object in topological dependency order as a
    /// portable bundle (`spec/07-ledger.md` §7.5.4), returning the count.
    ///
    /// # Errors
    ///
    /// [`StoreError::Corrupt`] if any object on disk fails verification,
    /// [`StoreError::Bundle`] if `objects/` contains a malformed filename, or
    /// [`StoreError::Io`] if reading or writing fails.
    pub fn export(&self, out: &mut impl io::Write) -> Result<usize, StoreError> {
        let mut malformed = BTreeSet::new();
        let files = self.scan_area("objects", &mut malformed)?;
        if !malformed.is_empty() {
            return Err(StoreError::Bundle("malformed path in objects/"));
        }
        if files.len() > MAX_BUNDLE_OBJECTS {
            return Err(StoreError::Bundle("bundle count exceeds bound"));
        }
        // Payload bytes are streamed from disk during emission rather than held in RAM.
        let mut found: BTreeMap<Cairn, (Vec<Cairn>, PathBuf)> = BTreeMap::new();
        let mut total_bytes: usize = 0;
        for (cairn, path) in files {
            let bytes = read_bounded(&path, None)?;
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .filter(|&t| t <= MAX_BUNDLE_BYTES)
                .ok_or(StoreError::Bundle("bundle exceeds total size bound"))?;
            let scanned = verify_with(cairn, &bytes, scan_stored)?;
            found.insert(cairn, (scanned.references, path));
        }
        let height = object_heights(&found)?;
        let mut entries: Vec<(Cairn, PathBuf)> =
            found.into_iter().map(|(c, (_, path))| (c, path)).collect();
        entries.sort_by_key(|(c, _)| (height[c], *c));
        let mut hasher = blake3::Hasher::new();
        let mut send = |slice: &[u8]| -> io::Result<()> {
            hasher.update(slice);
            out.write_all(slice)
        };
        send(BUNDLE_MAGIC)?;
        send(&(entries.len() as u64).to_be_bytes())?;
        for (cairn, path) in &entries {
            let bytes = read_bounded(path, None)?;
            verify_with(*cairn, &bytes, scan_stored)?;
            send(cairn.as_bytes())?;
            send(&(bytes.len() as u64).to_be_bytes())?;
            send(&bytes)?;
        }
        out.write_all(hasher.finalize().as_bytes())?;
        Ok(entries.len())
    }

    /// Imports a portable bundle (`spec/07-ledger.md` §7.5.4), staging verified
    /// raw entries in `tmp/` until the whole-stream trailer passes before
    /// committing objects and rebuilding `refs/`, returning the count.
    ///
    /// # Errors
    ///
    /// [`StoreError::Bundle`] if framing, bounds, ordering or trailer fail,
    /// [`StoreError::Corrupt`] if an entry or existing destination object fails
    /// verification, or [`StoreError::Io`] on I/O failure.
    pub fn import(&self, src: &mut impl io::Read) -> Result<usize, StoreError> {
        let mut hasher = blake3::Hasher::new();
        let mut hdr = [0u8; 24];
        read_hashed(src, &mut hasher, &mut hdr)?;
        if hdr != BUNDLE_MAGIC {
            return Err(StoreError::Bundle(
                if hdr.starts_with(b"netherc/cairn/") && hdr.ends_with(b"\x00bundle\x00") {
                    "incompatible bundle domain version"
                } else {
                    "not a store bundle"
                },
            ));
        }
        let mut u8s = [0u8; 8];
        read_hashed(src, &mut hasher, &mut u8s)?;
        let count = usize::try_from(u64::from_be_bytes(u8s))
            .ok()
            .filter(|&n| n <= MAX_BUNDLE_OBJECTS)
            .ok_or(StoreError::Bundle("bundle count exceeds bound"))?;
        let (stage_path, stage_file) = self.create_temp("import", true)?;
        let _guard = TempFileGuard::new(stage_path);
        let mut stage_out = io::BufWriter::new(stage_file);
        let mut seen = BTreeSet::new();
        let mut expected_before = BTreeSet::new();
        let mut total_bytes: usize = 0;
        for _ in 0..count {
            stage_bundle_entry(
                src,
                &mut hasher,
                &mut stage_out,
                &mut total_bytes,
                &mut seen,
                &mut expected_before,
            )?;
        }
        let mut trailer = [0u8; 32];
        read_bundle_exact(src, &mut trailer)?;
        if trailer != *hasher.finalize().as_bytes() {
            return Err(StoreError::Bundle("bundle trailer mismatch"));
        }
        loop {
            match src.read(&mut [0u8; 1]) {
                Ok(0) => break,
                Ok(_) => return Err(StoreError::Bundle("trailing bytes after bundle")),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(StoreError::Io(e)),
            }
        }
        stage_out.flush()?;
        let mut stage_file = stage_out.into_inner().map_err(io::IntoInnerError::into_error)?;
        stage_file.seek(SeekFrom::Start(0))?;
        self.commit_staged_bundle(&mut io::BufReader::new(stage_file), count)?;
        Ok(count)
    }

    fn commit_staged_bundle(
        &self,
        staged: &mut impl io::Read,
        count: usize,
    ) -> Result<(), StoreError> {
        let mut ref_cache: HashMap<Cairn, HashSet<Cairn>> = HashMap::new();
        for _ in 0..count {
            let mut raw = [0u8; 32];
            staged.read_exact(&mut raw)?;
            let cairn = Cairn::from_bytes(raw);
            let mut len_buf = [0u8; 8];
            staged.read_exact(&mut len_buf)?;
            let len = usize::try_from(u64::from_be_bytes(len_buf))
                .map_err(|_| StoreError::Bundle("bundle object exceeds size bound"))?;
            let mut bytes = vec![0u8; len];
            staged.read_exact(&mut bytes)?;
            let scanned = verify_with(cairn, &bytes, scan_stored)?;
            if self.has(cairn) {
                // §7.5.4: verify the on-disk object directly rather than relying
                // on the in-memory `self.seen` cache.
                let existing = read_bounded(&self.fanned("objects", cairn), Some(cairn))?;
                verify_with(cairn, &existing, scan_stored)?;
                for &target in &scanned.references {
                    let known = match ref_cache.entry(target) {
                        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                        std::collections::hash_map::Entry::Vacant(e) => {
                            e.insert(self.referrers(target)?.into_iter().collect())
                        }
                    };
                    if known.insert(cairn) {
                        self.add_ref(target, cairn)?;
                    }
                }
            } else {
                self.put_verified(cairn, &scanned.references, &bytes)?;
            }
        }
        Ok(())
    }
}

fn stage_bundle_entry(
    src: &mut impl io::Read,
    hasher: &mut blake3::Hasher,
    stage_out: &mut impl io::Write,
    total_bytes: &mut usize,
    seen: &mut BTreeSet<Cairn>,
    expected_before: &mut BTreeSet<Cairn>,
) -> Result<(), StoreError> {
    let mut raw = [0u8; 32];
    read_hashed(src, hasher, &mut raw)?;
    let cairn = Cairn::from_bytes(raw);
    if !seen.insert(cairn) {
        return Err(StoreError::Bundle("duplicate cairn in bundle"));
    }
    if expected_before.contains(&cairn) {
        return Err(StoreError::Bundle("bundle is not in topological order"));
    }
    let mut u8s = [0u8; 8];
    read_hashed(src, hasher, &mut u8s)?;
    let len = usize::try_from(u64::from_be_bytes(u8s))
        .ok()
        .filter(|&n| n <= MAX_OBJECT_BYTES)
        .ok_or(StoreError::Bundle("bundle object exceeds size bound"))?;
    *total_bytes = total_bytes
        .checked_add(len)
        .filter(|&t| t <= MAX_BUNDLE_BYTES)
        .ok_or(StoreError::Bundle("bundle exceeds total size bound"))?;
    // Read incrementally so a truncated stream cannot force a 64 MiB
    // pre-allocation before hitting EOF.
    let mut bytes = Vec::with_capacity(len.min(64 * 1024));
    src.by_ref().take(len as u64).read_to_end(&mut bytes)?;
    if bytes.len() != len {
        return Err(StoreError::Bundle("truncated bundle"));
    }
    hasher.update(&bytes);
    let scanned = verify_with(cairn, &bytes, scan_stored)?;
    for target in scanned.references {
        if !seen.contains(&target) {
            expected_before.insert(target);
        }
    }
    stage_out.write_all(cairn.as_bytes())?;
    stage_out.write_all(&(len as u64).to_be_bytes())?;
    stage_out.write_all(&bytes)?;
    Ok(())
}
