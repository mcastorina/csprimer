use std::iter;

// encode takes a value and returns an iterator of bytes.
pub fn encode(value: u128) -> impl Iterator<Item = u8> {
    let mut data = vec![];
    let mut bit_chunks = iter::successors(Some(value), |v| (*v > 0x7f).then_some(v >> 7))
        .map(|v| (v & 0x7f) as u8)
        .peekable();
    while let Some(mut b) = bit_chunks.next() {
        if bit_chunks.peek().is_some() {
            b |= 0x80;
        }
        data.push(b);
    }
    data.into_iter()
}

// decode reads from a stream of bytes and decodes the varint. If the stream is empty, None is
// returned.
pub fn decode(stream: &mut impl Iterator<Item = u8>) -> Option<u128> {
    take_until_inclusive(stream, |b| b & 0x80 != 0)
        .enumerate()
        .map(|(ofs, b)| ((b & 0x7f) as u128) << (ofs * 7))
        .reduce(|val, v| val | v)
}

// Like iter::take_until except it also includes the last element instead of discarding it.
fn take_until_inclusive<'a, I>(
    stream: &'a mut impl Iterator<Item = I>,
    mut pred: impl FnMut(&I) -> bool + 'a,
) -> impl Iterator<Item = I> + 'a {
    let mut done = false;
    iter::from_fn(move || {
        if done {
            return None;
        }
        let v = stream.next()?;
        done = !pred(&v);
        Some(v)
    })
}

#[cfg(test)]
mod test {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[test]
    fn test_encode_0() {
        let mut iter = encode(0);
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_encode_1() {
        let mut iter = encode(1);
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn test_encode_150() {
        let mut iter = encode(150);
        assert_eq!(iter.next(), Some(0x96));
        assert_eq!(iter.next(), Some(0x01));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn decode_multi() {
        let mut iter = [123, 456, 1337].iter().flat_map(|v| encode(*v));
        assert_eq!(decode(&mut iter), Some(123));
        assert_eq!(decode(&mut iter), Some(456));
        assert_eq!(decode(&mut iter), Some(1337));
        assert_eq!(decode(&mut iter), None);
    }

    #[quickcheck]
    fn test_encode_decode(input: u128) -> bool {
        decode(&mut encode(input)) == Some(input)
    }
}
