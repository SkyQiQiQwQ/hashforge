use digest::DynDigest;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

const BUFFER_SIZE: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Algorithm {
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha3_224,
    Sha3_256,
    Sha3_384,
    Sha3_512,
    Blake2b,
    Blake2s,
    Blake3,
    Crc32,
    Xxh3,
}

pub const ALGORITHMS: [Algorithm; 15] = [
    Algorithm::Md5,
    Algorithm::Sha1,
    Algorithm::Sha224,
    Algorithm::Sha256,
    Algorithm::Sha384,
    Algorithm::Sha512,
    Algorithm::Sha3_224,
    Algorithm::Sha3_256,
    Algorithm::Sha3_384,
    Algorithm::Sha3_512,
    Algorithm::Blake2b,
    Algorithm::Blake2s,
    Algorithm::Blake3,
    Algorithm::Crc32,
    Algorithm::Xxh3,
];

pub const DEFAULT_ALGORITHM: Algorithm = Algorithm::Sha256;

impl Algorithm {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Md5 => "MD5",
            Self::Sha1 => "SHA-1",
            Self::Sha224 => "SHA-224",
            Self::Sha256 => "SHA-256",
            Self::Sha384 => "SHA-384",
            Self::Sha512 => "SHA-512",
            Self::Sha3_224 => "SHA3-224",
            Self::Sha3_256 => "SHA3-256",
            Self::Sha3_384 => "SHA3-384",
            Self::Sha3_512 => "SHA3-512",
            Self::Blake2b => "BLAKE2B",
            Self::Blake2s => "BLAKE2S",
            Self::Blake3 => "BLAKE3",
            Self::Crc32 => "CRC32",
            Self::Xxh3 => "XXH3",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha224 => "sha224",
            Self::Sha256 => "sha256",
            Self::Sha384 => "sha384",
            Self::Sha512 => "sha512",
            Self::Sha3_224 => "sha3-224",
            Self::Sha3_256 => "sha3-256",
            Self::Sha3_384 => "sha3-384",
            Self::Sha3_512 => "sha3-512",
            Self::Blake2b => "blake2b",
            Self::Blake2s => "blake2s",
            Self::Blake3 => "blake3",
            Self::Crc32 => "crc32",
            Self::Xxh3 => "xxh3",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().replace('_', "-").as_str() {
            "md5" => Some(Self::Md5),
            "sha1" | "sha-1" => Some(Self::Sha1),
            "sha224" | "sha-224" => Some(Self::Sha224),
            "sha256" | "sha-256" => Some(Self::Sha256),
            "sha384" | "sha-384" => Some(Self::Sha384),
            "sha512" | "sha-512" => Some(Self::Sha512),
            "sha3-224" => Some(Self::Sha3_224),
            "sha3-256" => Some(Self::Sha3_256),
            "sha3-384" => Some(Self::Sha3_384),
            "sha3-512" => Some(Self::Sha3_512),
            "blake2b" => Some(Self::Blake2b),
            "blake2s" => Some(Self::Blake2s),
            "blake3" => Some(Self::Blake3),
            "crc32" => Some(Self::Crc32),
            "xxh3" | "xxh3-64" => Some(Self::Xxh3),
            _ => None,
        }
    }
}

enum Hasher {
    Digest(Box<dyn DynDigest>),
    Blake3(Box<blake3::Hasher>),
    Crc32(crc32fast::Hasher),
    Xxh3(Box<xxhash_rust::xxh3::Xxh3>),
}

impl Hasher {
    fn new(algorithm: Algorithm) -> Self {
        use digest::Digest;

        match algorithm {
            Algorithm::Md5 => Self::Digest(Box::new(md5::Md5::new())),
            Algorithm::Sha1 => Self::Digest(Box::new(sha1::Sha1::new())),
            Algorithm::Sha224 => Self::Digest(Box::new(sha2::Sha224::new())),
            Algorithm::Sha256 => Self::Digest(Box::new(sha2::Sha256::new())),
            Algorithm::Sha384 => Self::Digest(Box::new(sha2::Sha384::new())),
            Algorithm::Sha512 => Self::Digest(Box::new(sha2::Sha512::new())),
            Algorithm::Sha3_224 => Self::Digest(Box::new(sha3::Sha3_224::new())),
            Algorithm::Sha3_256 => Self::Digest(Box::new(sha3::Sha3_256::new())),
            Algorithm::Sha3_384 => Self::Digest(Box::new(sha3::Sha3_384::new())),
            Algorithm::Sha3_512 => Self::Digest(Box::new(sha3::Sha3_512::new())),
            Algorithm::Blake2b => Self::Digest(Box::new(blake2::Blake2b512::new())),
            Algorithm::Blake2s => Self::Digest(Box::new(blake2::Blake2s256::new())),
            Algorithm::Blake3 => Self::Blake3(Box::new(blake3::Hasher::new())),
            Algorithm::Crc32 => Self::Crc32(crc32fast::Hasher::new()),
            Algorithm::Xxh3 => Self::Xxh3(Box::new(xxhash_rust::xxh3::Xxh3::new())),
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Digest(hasher) => hasher.update(bytes),
            Self::Blake3(hasher) => {
                hasher.update(bytes);
            }
            Self::Crc32(hasher) => hasher.update(bytes),
            Self::Xxh3(hasher) => {
                hasher.update(bytes);
            }
        }
    }

    fn finish(self) -> String {
        let bytes = match self {
            Self::Digest(hasher) => hasher.finalize().into_vec(),
            Self::Blake3(hasher) => hasher.finalize().as_bytes().to_vec(),
            Self::Crc32(hasher) => hasher.finalize().to_be_bytes().to_vec(),
            Self::Xxh3(hasher) => hasher.digest().to_be_bytes().to_vec(),
        };
        hex_encode(&bytes)
    }
}

fn hash_reader<R: Read>(
    reader: &mut R,
    algorithms: &[Algorithm],
) -> io::Result<Vec<(Algorithm, String)>> {
    let mut hashers: Vec<_> = algorithms
        .iter()
        .copied()
        .map(|algorithm| (algorithm, Hasher::new(algorithm)))
        .collect();
    let mut buffer = vec![0_u8; BUFFER_SIZE];

    loop {
        let read = match reader.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if read == 0 {
            break;
        }
        for (_, hasher) in &mut hashers {
            hasher.update(&buffer[..read]);
        }
    }

    Ok(hashers
        .into_iter()
        .map(|(algorithm, hasher)| (algorithm, hasher.finish()))
        .collect())
}

pub fn hash_bytes(bytes: &[u8], algorithms: &[Algorithm]) -> Vec<(Algorithm, String)> {
    let mut hashers: Vec<_> = algorithms
        .iter()
        .copied()
        .map(|algorithm| (algorithm, Hasher::new(algorithm)))
        .collect();
    for (_, hasher) in &mut hashers {
        hasher.update(bytes);
    }
    hashers
        .into_iter()
        .map(|(algorithm, hasher)| (algorithm, hasher.finish()))
        .collect()
}

pub fn hash_file(
    path: &Path,
    algorithms: &[Algorithm],
) -> Result<Vec<(Algorithm, String)>, String> {
    if path.as_os_str() == OsStr::new("-") {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        return hash_reader(&mut input, algorithms)
            .map_err(|error| format!("failed to read standard input: {error}"));
    }
    let mut file = File::open(path)
        .map_err(|error| format!("failed to open '{}': {error}", path.display()))?;
    hash_reader(&mut file, algorithms)
        .map_err(|error| format!("failed to read '{}': {error}", path.display()))
}

pub fn validate_hex(value: &str, algorithm: Algorithm) -> Result<(), String> {
    let expected_length = match algorithm {
        Algorithm::Md5 => 32,
        Algorithm::Sha1 => 40,
        Algorithm::Sha224 | Algorithm::Sha3_224 => 56,
        Algorithm::Sha256 | Algorithm::Sha3_256 | Algorithm::Blake2s | Algorithm::Blake3 => 64,
        Algorithm::Sha384 | Algorithm::Sha3_384 => 96,
        Algorithm::Sha512 | Algorithm::Sha3_512 | Algorithm::Blake2b => 128,
        Algorithm::Crc32 => 8,
        Algorithm::Xxh3 => 16,
    };
    if value.len() != expected_length || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "expected a {expected_length}-character hexadecimal {} digest",
            algorithm.name()
        ));
    }
    Ok(())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn digest(algorithm: Algorithm, input: &[u8]) -> String {
        hash_bytes(input, &[algorithm])[0].1.clone()
    }

    #[test]
    fn known_digest_vectors_match() {
        assert_eq!(
            digest(Algorithm::Md5, b""),
            "d41d8cd98f00b204e9800998ecf8427e"
        );
        assert_eq!(
            digest(Algorithm::Sha1, b"abc"),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
        assert_eq!(
            digest(Algorithm::Sha224, b"abc"),
            "23097d223405d8228642a477bda255b32aadbce4bda0b3f7e36c9da7"
        );
        assert_eq!(
            digest(Algorithm::Sha256, b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(digest(Algorithm::Sha384, b"abc"), "cb00753f45a35e8bb5a03d699ac65007272c32ab0eded1631a8b605a43ff5bed8086072ba1e7cc2358baeca134c825a7");
        assert_eq!(digest(Algorithm::Sha512, b"abc"), "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f");
        assert_eq!(
            digest(Algorithm::Sha3_224, b"abc"),
            "e642824c3f8cf24ad09234ee7d3c766fc9a3a5168d0c94ad73b46fdf"
        );
        assert_eq!(
            digest(Algorithm::Sha3_256, b"abc"),
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
        );
        assert_eq!(digest(Algorithm::Sha3_384, b"abc"), "ec01498288516fc926459f58e2c6ad8df9b473cb0fc08c2596da7cf0e49be4b298d88cea927ac7f539f1edf228376d25");
        assert_eq!(digest(Algorithm::Sha3_512, b"abc"), "b751850b1a57168a5693cd924b6b096e08f621827444f70d884f5d0240d2712e10e116e9192af3c91a7ec57647e3934057340b4cf408d5a56592f8274eec53f0");
        assert_eq!(
            digest(Algorithm::Blake3, b""),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        assert_eq!(digest(Algorithm::Blake2b, b""), "786a02f742015903c6c6fd852552d272912f4740e15847618a86e217f71f5419d25e1031afee585313896444934eb04b903a685b1448b755d56f701afe9be2ce");
        assert_eq!(
            digest(Algorithm::Blake2s, b""),
            "69217a3079908094e11121d042354a7c1f55b6482ca1a51e1b250dfd1ed0eef9"
        );
        assert_eq!(digest(Algorithm::Crc32, b"123456789"), "cbf43926");
        assert_eq!(digest(Algorithm::Xxh3, b""), "2d06800538d394c2");
    }

    #[test]
    fn streaming_multi_hash_matches_one_shot_for_all_algorithms() {
        let input = vec![b'x'; BUFFER_SIZE + 37];
        let expected = hash_bytes(&input, &ALGORITHMS);
        let mut reader = Cursor::new(input);
        let streamed = hash_reader(&mut reader, &ALGORITHMS).unwrap();
        assert_eq!(streamed, expected);
    }

    #[test]
    fn interrupted_reads_are_retried() {
        struct InterruptOnce {
            interrupted: bool,
            input: Cursor<&'static [u8]>,
        }

        impl Read for InterruptOnce {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                self.input.read(buffer)
            }
        }

        let mut reader = InterruptOnce {
            interrupted: false,
            input: Cursor::new(&b"abc"[..]),
        };
        assert_eq!(
            hash_reader(&mut reader, &[Algorithm::Sha256]).unwrap(),
            hash_bytes(b"abc", &[Algorithm::Sha256])
        );
    }

    #[test]
    fn output_lengths_and_verification_lengths_match() {
        let lengths = [32, 40, 56, 64, 96, 128, 56, 64, 96, 128, 128, 64, 64, 8, 16];
        for (algorithm, expected_length) in ALGORITHMS.into_iter().zip(lengths) {
            let value = digest(algorithm, b"abc");
            assert_eq!(value.len(), expected_length, "{}", algorithm.name());
            validate_hex(&value, algorithm).unwrap();
        }
    }

    #[test]
    fn default_algorithm_is_sha256() {
        assert_eq!(DEFAULT_ALGORITHM, Algorithm::Sha256);
    }
}
