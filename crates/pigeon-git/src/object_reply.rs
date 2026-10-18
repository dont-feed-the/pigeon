//! Complete ordered replies from `git cat-file --batch` (03 §3.7).
//! An incomplete reading must never become an authoritative smaller inventory.

/// One optional object per request, retaining its kind and exact bytes.
pub type Objects = Vec<Option<(String, Vec<u8>)>>;

/// Decode one complete native batch reply.
pub fn parse(bytes: &[u8], names: &[String]) -> Result<Objects, &'static str> {
    let mut rest = bytes;
    let mut found = Vec::with_capacity(names.len());
    for name in names {
        let header = header(&mut rest)?;
        if header
            .strip_suffix(" missing")
            .or_else(|| header.strip_suffix(" ambiguous"))
            == Some(name.as_str())
        {
            found.push(None);
            continue;
        }
        let (_, kind, size) = metadata(header, name)?;
        let end = size
            .checked_add(1)
            .ok_or("object size overflows its frame")?;
        let frame = rest.get(..end).ok_or("incomplete object payload")?;
        if frame.last() != Some(&b'\n') {
            return Err("missing object payload delimiter");
        }
        found.push(Some((kind, frame[..size].to_vec())));
        rest = &rest[end..];
    }
    if !rest.is_empty() {
        return Err("unrequested object replies or trailing bytes");
    }
    Ok(found)
}

fn header<'a>(rest: &mut &'a [u8]) -> Result<&'a str, &'static str> {
    let nl = rest
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or("incomplete object header")?;
    let line = std::str::from_utf8(&rest[..nl]).map_err(|_| "invalid object header encoding")?;
    *rest = &rest[nl + 1..];
    Ok(line)
}

fn metadata(line: &str, name: &str) -> Result<(String, String, usize), &'static str> {
    let Some((oid, kind, size)) =
        crate::catfile::parse_header(line).map_err(|_| "malformed object header")?
    else {
        return Err("missing or ambiguous requested object");
    };
    let object_id =
        |text: &str| matches!(text.len(), 40 | 64) && text.bytes().all(|b| b.is_ascii_hexdigit());
    if !object_id(&oid) || (object_id(name) && !oid.eq_ignore_ascii_case(name)) {
        return Err("object reply does not match its request");
    }
    if !matches!(kind.as_str(), "blob" | "tree" | "commit" | "tag") {
        return Err("unknown object kind");
    }
    Ok((oid, kind, size))
}

/// Complete ordered metadata for retained-history size selection. A missing
/// object is an error here, never a reason to omit it from the inventory.
pub fn check(bytes: &[u8], names: &[String]) -> Result<Vec<(String, usize)>, &'static str> {
    let mut rest = bytes;
    let mut found = Vec::with_capacity(names.len());
    for name in names {
        let (_, kind, size) = metadata(header(&mut rest)?, name)?;
        found.push((kind, size));
    }
    if !rest.is_empty() {
        return Err("unrequested metadata replies or trailing bytes");
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hegel::generators as gs;

    #[hegel::test(test_cases = 1000)]
    fn complete_binary_payloads_round_trip_without_interpreting_their_framing(tc: hegel::TestCase) {
        let payloads = tc.draw(gs::vecs(gs::binary().max_size(256)).max_size(8));
        let names: Vec<_> = (0..payloads.len()).map(|i| format!("{i:040x}")).collect();
        let mut wire = Vec::new();
        for (name, data) in names.iter().zip(&payloads) {
            wire.extend_from_slice(format!("{name} blob {}\n", data.len()).as_bytes());
            wire.extend_from_slice(data);
            wire.push(b'\n');
        }
        let expected: Objects = payloads
            .into_iter()
            .map(|data| Some(("blob".into(), data)))
            .collect();
        assert_eq!(parse(&wire, &names), Ok(expected));
    }

    #[hegel::test(test_cases = 1000)]
    fn metadata_is_complete_and_ordered_for_every_requested_object(tc: hegel::TestCase) {
        let sizes = tc.draw(gs::vecs(gs::integers::<u16>()).min_size(1).max_size(12));
        let names: Vec<_> = (0..sizes.len()).map(|i| format!("{i:040x}")).collect();
        let wire = names
            .iter()
            .zip(&sizes)
            .map(|(name, size)| format!("{name} blob {size}\n"))
            .collect::<String>();
        let expected = sizes
            .into_iter()
            .map(|size| ("blob".to_owned(), usize::from(size)))
            .collect::<Vec<_>>();
        assert_eq!(check(wire.as_bytes(), &names), Ok(expected));
        let cut = tc.draw(gs::integers::<usize>().max_value(wire.len() - 1));
        assert!(check(&wire.as_bytes()[..cut], &names).is_err());
        if names.len() > 1 {
            let mut reordered = names;
            reordered.swap(0, 1);
            assert!(check(wire.as_bytes(), &reordered).is_err());
        }
    }

    #[hegel::test(test_cases = 1000)]
    fn every_strict_prefix_of_an_object_frame_is_incomplete(tc: hegel::TestCase) {
        let data = tc.draw(gs::binary().max_size(256));
        let name = "a".repeat(40);
        let mut wire = format!("{name} blob {}\n", data.len()).into_bytes();
        wire.extend_from_slice(&data);
        wire.push(b'\n');
        let cut = tc.draw(gs::integers::<usize>().max_value(wire.len() - 1));
        assert!(parse(&wire[..cut], &[name]).is_err());
    }

    #[hegel::test(test_cases = 1000)]
    fn arbitrary_extra_bytes_cannot_extend_a_complete_reading(tc: hegel::TestCase) {
        let extra = tc.draw(gs::binary().min_size(1).max_size(64));
        let name = "a".repeat(40);
        let mut wire = format!("{name} blob 0\n\n").into_bytes();
        wire.extend_from_slice(&extra);
        assert!(parse(&wire, &[name]).is_err());
    }

    #[test]
    fn explicit_missing_replies_preserve_the_request_position() {
        let names = vec!["main:missing file".to_owned(), "a".repeat(40)];
        let wire = format!("{} missing\n{} blob 1\nx\n", names[0], names[1]);
        assert_eq!(
            parse(wire.as_bytes(), &names),
            Ok(vec![None, Some(("blob".into(), b"x".to_vec()))])
        );
        assert!(parse(b"another missing\n", &names[..1]).is_err());
        assert!(check(wire.as_bytes(), &names).is_err());
    }

    #[test]
    fn ordered_metadata_requires_valid_sizes_and_matching_object_ids() {
        let names = vec!["a".repeat(40), "b".repeat(64)];
        let valid = format!("{} blob 1024\n{} tree 4096\n", names[0], names[1]);
        assert_eq!(
            check(valid.as_bytes(), &names),
            Ok(vec![("blob".into(), 1024), ("tree".into(), 4096)])
        );
        for wire in [
            format!("{} blob nope\n{} tree 4096\n", names[0], names[1]),
            format!("{} blob 1024\n", names[0]),
            format!("{} tree 4096\n{} blob 1024\n", names[1], names[0]),
            valid + "extra\n",
        ] {
            assert!(check(wire.as_bytes(), &names).is_err());
        }
        let mismatched = format!("{} blob 0\n\n", names[1]);
        assert!(parse(mismatched.as_bytes(), &names[..1]).is_err());
        let oversized = format!("{} blob {}\n", names[0], usize::MAX);
        assert!(parse(oversized.as_bytes(), &names[..1]).is_err());
    }

    #[test]
    fn an_incomplete_reply_cannot_silently_remove_a_requested_object() {
        let oid = "a".repeat(40);
        let reply = format!("{oid} blob 1\nx\n");
        assert!(parse(reply.as_bytes(), &[oid.clone(), oid]).is_err());
    }

    #[test]
    fn a_truncated_payload_is_not_an_empty_object() {
        let oid = "a".repeat(40);
        let reply = format!("{oid} blob 5\nxx");
        assert!(parse(reply.as_bytes(), &[oid]).is_err());
    }

    #[test]
    fn malformed_headers_are_not_missing_objects() {
        assert!(parse(b"not a header\n", &["a".repeat(40)]).is_err());
    }

    #[test]
    fn each_payload_requires_its_exact_trailing_delimiter() {
        let oid = "a".repeat(40);
        let reply = format!("{oid} blob 1\nx!");
        assert!(parse(reply.as_bytes(), &[oid]).is_err());
    }

    #[test]
    fn extra_replies_cannot_shift_the_request_to_object_binding() {
        let oid = "a".repeat(40);
        let reply = format!("{oid} blob 1\nx\n{oid} blob 1\ny\n");
        assert!(parse(reply.as_bytes(), &[oid]).is_err());
    }
}
