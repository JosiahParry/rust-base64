use crate::engine::Engine;
#[cfg(any(feature = "alloc", feature = "std", test))]
use crate::engine::DEFAULT_ENGINE;
#[cfg(any(feature = "alloc", feature = "std", test))]
use alloc::vec::Vec;
use core::fmt;
#[cfg(any(feature = "std", test))]
use std::error;

/// Errors that can occur while decoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// An invalid byte was found in the input. The offset and offending byte are provided.
    /// Padding characters (`=`) interspersed in the encoded form will be treated as invalid bytes.
    InvalidByte(usize, u8),
    /// The length of the input is invalid.
    /// A typical cause of this is stray trailing whitespace or other separator bytes.
    /// In the case where excess trailing bytes have produced an invalid length *and* the last byte
    /// is also an invalid base64 symbol (as would be the case for whitespace, etc), `InvalidByte`
    /// will be emitted instead of `InvalidLength` to make the issue easier to debug.
    InvalidLength,
    /// The last non-padding input symbol's encoded 6 bits have nonzero bits that will be discarded.
    /// This is indicative of corrupted or truncated Base64.
    /// Unlike `InvalidByte`, which reports symbols that aren't in the alphabet, this error is for
    /// symbols that are in the alphabet but represent nonsensical encodings.
    InvalidLastSymbol(usize, u8),
    /// The nature of the padding was not as configured: absent or incorrect when it must be
    /// canonical, or present when it must be absent, etc.
    InvalidPadding,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Self::InvalidByte(index, byte) => write!(f, "Invalid byte {}, offset {}.", byte, index),
            Self::InvalidLength => write!(f, "Encoded text cannot have a 6-bit remainder."),
            Self::InvalidLastSymbol(index, byte) => {
                write!(f, "Invalid last symbol {}, offset {}.", byte, index)
            }
            Self::InvalidPadding => write!(f, "Invalid padding"),
        }
    }
}

#[cfg(any(feature = "std", test))]
impl error::Error for DecodeError {
    fn description(&self) -> &str {
        match *self {
            Self::InvalidByte(_, _) => "invalid byte",
            Self::InvalidLength => "invalid length",
            Self::InvalidLastSymbol(_, _) => "invalid last symbol",
            Self::InvalidPadding => "invalid padding",
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        None
    }
}

///Decode base64 using the [default engine](DEFAULT_ENGINE).
///
/// See [Engine::decode].
#[deprecated(since = "0.21.0", note = "Use Engine::decode")]
#[cfg(any(feature = "alloc", feature = "std", test))]
pub fn decode<T: AsRef<[u8]>>(input: T) -> Result<Vec<u8>, DecodeError> {
    DEFAULT_ENGINE.decode(input)
}

///Decode from string reference as octets using the specified [Engine].
///
/// See [Engine::decode].
///Returns a `Result` containing a `Vec<u8>`.
#[deprecated(since = "0.21.0", note = "Use Engine::decode")]
#[cfg(any(feature = "alloc", feature = "std", test))]
pub fn decode_engine<E: Engine, T: AsRef<[u8]>>(
    input: T,
    engine: &E,
) -> Result<Vec<u8>, DecodeError> {
    engine.decode(input)
}

/// Decode from string reference as octets.
///
/// See [Engine::decode_vec].
#[cfg(any(feature = "alloc", feature = "std", test))]
#[deprecated(since = "0.21.0", note = "Use Engine::decode_vec")]
pub fn decode_engine_vec<E: Engine, T: AsRef<[u8]>>(
    input: T,
    buffer: &mut Vec<u8>,
    engine: &E,
) -> Result<(), DecodeError> {
    engine.decode_vec(input, buffer)
}

/// Decode the input into the provided output slice.
///
/// See [Engine::decode_slice].
#[deprecated(since = "0.21.0", note = "Use Engine::decode_slice")]
pub fn decode_engine_slice<E: Engine, T: AsRef<[u8]>>(
    input: T,
    output: &mut [u8],
    engine: &E,
) -> Result<usize, DecodeError> {
    engine.decode_slice(input, output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        alphabet,
        engine::{general_purpose, Config, GeneralPurpose},
        tests::{assert_encode_sanity, random_engine},
    };
    use rand::{
        distributions::{Distribution, Uniform},
        Rng, SeedableRng,
    };

    #[test]
    fn decode_into_nonempty_vec_doesnt_clobber_existing_prefix() {
        let mut orig_data = Vec::new();
        let mut encoded_data = String::new();
        let mut decoded_with_prefix = Vec::new();
        let mut decoded_without_prefix = Vec::new();
        let mut prefix = Vec::new();

        let prefix_len_range = Uniform::new(0, 1000);
        let input_len_range = Uniform::new(0, 1000);

        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..10_000 {
            orig_data.clear();
            encoded_data.clear();
            decoded_with_prefix.clear();
            decoded_without_prefix.clear();
            prefix.clear();

            let input_len = input_len_range.sample(&mut rng);

            for _ in 0..input_len {
                orig_data.push(rng.gen());
            }

            let engine = random_engine(&mut rng);
            engine.encode_string(&orig_data, &mut encoded_data);
            assert_encode_sanity(&encoded_data, engine.config().encode_padding(), input_len);

            let prefix_len = prefix_len_range.sample(&mut rng);

            // fill the buf with a prefix
            for _ in 0..prefix_len {
                prefix.push(rng.gen());
            }

            decoded_with_prefix.resize(prefix_len, 0);
            decoded_with_prefix.copy_from_slice(&prefix);

            // decode into the non-empty buf
            engine
                .decode_vec(&encoded_data, &mut decoded_with_prefix)
                .unwrap();
            // also decode into the empty buf
            engine
                .decode_vec(&encoded_data, &mut decoded_without_prefix)
                .unwrap();

            assert_eq!(
                prefix_len + decoded_without_prefix.len(),
                decoded_with_prefix.len()
            );
            assert_eq!(orig_data, decoded_without_prefix);

            // append plain decode onto prefix
            prefix.append(&mut decoded_without_prefix);

            assert_eq!(prefix, decoded_with_prefix);
        }
    }

    #[test]
    fn decode_into_slice_doesnt_clobber_existing_prefix_or_suffix() {
        let mut orig_data = Vec::new();
        let mut encoded_data = String::new();
        let mut decode_buf = Vec::new();
        let mut decode_buf_copy: Vec<u8> = Vec::new();

        let input_len_range = Uniform::new(0, 1000);

        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..10_000 {
            orig_data.clear();
            encoded_data.clear();
            decode_buf.clear();
            decode_buf_copy.clear();

            let input_len = input_len_range.sample(&mut rng);

            for _ in 0..input_len {
                orig_data.push(rng.gen());
            }

            let engine = random_engine(&mut rng);
            engine.encode_string(&orig_data, &mut encoded_data);
            assert_encode_sanity(&encoded_data, engine.config().encode_padding(), input_len);

            // fill the buffer with random garbage, long enough to have some room before and after
            for _ in 0..5000 {
                decode_buf.push(rng.gen());
            }

            // keep a copy for later comparison
            decode_buf_copy.extend(decode_buf.iter());

            let offset = 1000;

            // decode into the non-empty buf
            let decode_bytes_written = engine
                .decode_slice(&encoded_data, &mut decode_buf[offset..])
                .unwrap();

            assert_eq!(orig_data.len(), decode_bytes_written);
            assert_eq!(
                orig_data,
                &decode_buf[offset..(offset + decode_bytes_written)]
            );
            assert_eq!(&decode_buf_copy[0..offset], &decode_buf[0..offset]);
            assert_eq!(
                &decode_buf_copy[offset + decode_bytes_written..],
                &decode_buf[offset + decode_bytes_written..]
            );
        }
    }

    #[test]
    fn decode_into_slice_fits_in_precisely_sized_slice() {
        let mut orig_data = Vec::new();
        let mut encoded_data = String::new();
        let mut decode_buf = Vec::new();

        let input_len_range = Uniform::new(0, 1000);
        let mut rng = rand::rngs::SmallRng::from_entropy();

        for _ in 0..10_000 {
            orig_data.clear();
            encoded_data.clear();
            decode_buf.clear();

            let input_len = input_len_range.sample(&mut rng);

            for _ in 0..input_len {
                orig_data.push(rng.gen());
            }

            let engine = random_engine(&mut rng);
            engine.encode_string(&orig_data, &mut encoded_data);
            assert_encode_sanity(&encoded_data, engine.config().encode_padding(), input_len);

            decode_buf.resize(input_len, 0);

            // decode into the non-empty buf
            let decode_bytes_written = engine
                .decode_slice(&encoded_data, &mut decode_buf[..])
                .unwrap();

            assert_eq!(orig_data.len(), decode_bytes_written);
            assert_eq!(orig_data, decode_buf);
        }
    }

    #[test]
    fn decode_engine_estimation_works_for_various_lengths() {
        let engine = GeneralPurpose::new(&alphabet::STANDARD, general_purpose::NO_PAD);
        for num_prefix_quads in 0..100 {
            for suffix in &["AA", "AAA", "AAAA"] {
                let mut prefix = "AAAA".repeat(num_prefix_quads);
                prefix.push_str(suffix);
                // make sure no overflow (and thus a panic) occurs
                let res = engine.decode(prefix);
                assert!(res.is_ok());
            }
        }
    }
}
