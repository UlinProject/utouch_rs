use log::trace;

use crate::config::INVALID_BYTE;
pub type TouchChunk = [u8; 7];

pub struct BuildReader {
	address: [u8; 2],
	is_write_address: u8, // 0 - true, 1 - true, 2 - false,

	buff: Vec<u8>,

	is_write_wlen: bool,
	wlen: u8,

	end_byte: u8,
	dwarn: bool,

	c_unk_bytes: u8,
}

impl BuildReader {
	#[inline]
	pub fn empty() -> Self {
		Self::with_capacity(125)
	}

	pub fn with_capacity(size: usize) -> Self {
		Self {
			address: Default::default(),
			is_write_address: 0,

			buff: Vec::with_capacity(size),

			is_write_wlen: true,
			wlen: 0,
			end_byte: 0,

			dwarn: false,
			c_unk_bytes: 0,
		}
	}

	pub fn clear(&mut self) {
		self.is_write_address = 0;
		self.buff.clear();
		self.is_write_wlen = true;

		self.wlen = 0;
		self.end_byte = 0;

		self.dwarn = false;
		self.c_unk_bytes = 0;
	}

	#[allow(dead_code)]
	#[inline]
	pub const fn is_maybe_write(&self) -> bool {
		!self.dwarn
	}

	#[inline]
	pub fn get_line(&self) -> (u16, &[u8], u8) {
		(
			u16::from_le_bytes(self.address),
			self.buff.as_slice(),
			self.end_byte,
		)
	}

	pub fn write(&mut self, abyte: u8) -> BuildReaderRes {
		if (self.is_write_address as usize) < self.address.len() {
			if abyte == INVALID_BYTE {
				self.c_unk_bytes += 1;
				if self.c_unk_bytes >= 200 {
					self.c_unk_bytes = 0;
					return BuildReaderRes::IgnoreAndSkipData;
				}
				return BuildReaderRes::Ignore;
			}

			//trace!("waddres, {:?}", abyte);
			self.address[self.is_write_address as usize] = abyte;
			self.is_write_address += 1;

			return BuildReaderRes::Continue;
		}

		if self.is_write_wlen {
			// WRITE LEN
			self.wlen = abyte as _;
			self.is_write_wlen = false;

			//trace!("wlen, {:?}", self.is_write_wlen);
			return BuildReaderRes::Continue;
		}

		if self.wlen > 0 {
			// LEN
			self.buff.push(abyte);
			self.wlen -= 1;

			return BuildReaderRes::Continue;
		}

		if self.dwarn {
			trace!("#warn 53, cycle write builder, exp clear");
			return BuildReaderRes::Ignore;
		}

		self.end_byte = abyte;
		self.dwarn = true;

		BuildReaderRes::EndLine
	}
}

#[derive(Debug, Clone, Copy)]
pub enum BuildReaderRes {
	EndLine,
	Ignore,
	IgnoreAndSkipData,

	Continue,
}

impl BuildReaderRes {
	#[inline]
	pub const fn is_end_line(&self) -> bool {
		matches!(self, Self::EndLine)
	}

	#[inline]
	pub const fn is_continue(&self) -> bool {
		matches!(self, Self::Continue)
	}

	#[inline]
	pub const fn is_ignore(&self) -> bool {
		matches!(self, Self::Ignore)
	}

	#[inline]
	pub const fn is_ignore_and_skipdata(&self) -> bool {
		matches!(self, Self::IgnoreAndSkipData)
	}
}

pub struct Reader {}

impl Reader {
	pub fn search(mut iter: impl Iterator<Item = u8>, mut next: impl FnMut(&mut TouchChunk)) {
		let mut tchunk = TouchChunk::default();

		'sbegin: {
			match iter.next() {
				Some(0) => {} // OK,
				Some(a) => println!("#warn 74 unk_start_byte, {:?}", a),
				None => break 'sbegin,
			}
			loop {
				let mut write_chunk_len = 0;
				for a_write in tchunk.iter_mut() {
					match iter.next() {
						Some(a) => {
							write_chunk_len += 1;
							*a_write = a;
						}
						None => {
							if write_chunk_len > 0 {
								println!(
									"#warn 110, invalid chunk, {:?}",
									tchunk.get(..write_chunk_len)
								);
							}
							break 'sbegin;
						}
					}
				}
				//trace!("{:#01x}[{}] {}: {:?}", u16::from_le_bytes(address), a_num, num, chunk);
				next(&mut tchunk);
			}
		}
	}
}

// cargo test -- --nocapture
#[cfg(test)]
#[test]
fn check_model1() {
	let inarray = [
		// EMPTY STR
		0xA5, 0x11, 0x0, 0x0, // 0
		0xA5, 0x11, 8, 0x0, 0x0, 16, 0x4, 0x31, 0x4, 0xC, 0x40, 0x0, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A,
		0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A,
		0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, // 1
		0xA5, 0x11, 15, 0x0, 0x0, 16, 0x4, 0x31, 0x4, 0x10, 0x50, 0x0, 0x11, 0x51, 0xF2, 0x1C, 0x8,
		0x80, 0x0, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A,
		0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, // 2
		0xA5, 0x11, 15, 0x0, 0x0, 16, 0xFF, 0x30, 0x4, 0x10, 0x50, 0x0, 0x11, 0x51, 0xF2, 0x1C,
		0x8, 0x90, 0x0, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A,
		0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, // 3
		0xA5, 0x11, 22, 0x0, 0x0, 16, 0xF9, 0x30, 0x4, 0x10, 0x50, 0x0, 0x11, 0x51, 0xF2, 0x1C,
		0x8, 0x90, 0x0, 0x12, 0x50, 0xF2, 0x33, 0x8, 0x80, 0x0, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A,
		0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, // EMPTY STR
		0xA5, 0x11, 0x0, 0x0, // 4
		0xA5, 0x11, 29, 0x0, 0x0, 16, 0xF6, 0x30, 0x4, 0x10, 0x50, 0x0, 0x11, 0x51, 0xF2, 0x1C,
		0x9, 0x90, 0x0, 0x12, 0x50, 0xF2, 0x33, 0x9, 0x90, 0x0, 0x13, 0x97, 0xF1, 0x3E, 0x8, 0x90,
		0x0, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, 0x5A, // 5
		0xA5, 0x11, 36, 0x0, 0x0, 16, 0xF7, 0x30, 0x4, 0x10, 0x50, 0x0, 0x11, 0x51, 0xF2, 0x1C,
		0x9, 0x90, 0x0, 0x12, 0x50, 0xF2, 0x33, 0x9, 0x90, 0x0, 0x13, 0x97, 0xF1, 0x3E, 0x8, 0x90,
		0x0, 0x14, 0x40, 0x72, 0x3F, 0x3, 0x80, 0x0, 0x5A, 0x5A,
	];

	let mut builder = BuildReader::empty();
	for a in inarray.into_iter() {
		let result = builder.write(a);
		if result.is_end_line() {
			let (address, line, endb) = builder.get_line();

			println!("#line {:?}", line);
			println!("#endbyte {:?}", endb);
			Reader::search(
				line.iter().copied(),
				|/*address,*/ /*n_message*/ /*, n_chunk*/ data| {
					println!("{:#01x}: {:?}", address /*, n_chunk*/, data);
				},
			);

			builder.clear();
			continue;
		}
		if result.is_ignore() {
			continue;
		}
		if result.is_ignore_and_skipdata() {
			break;
		}
	}
}

#[cfg(test)]
#[test]
fn check_model2() {
	let inarray = [
		0xA5, 16, 24, 0x0, 0x1, 0b1, 0x73, 0x33, 0x39, 0x30, 0x38, 45, 0x31, 0x35, 0x2E, 0x30,
		0x2E, 48, 0, 0, 0, 0, 0xB2, 69, 52, 0, 0, 4, 90, 90, 90, 90, 90, 90,
	];

	let mut builder = BuildReader::empty();
	for a in inarray.into_iter() {
		let result = builder.write(a);
		if result.is_end_line() {
			let (address, line, endb) = builder.get_line();

			println!("#line {:?}", line);
			println!("#endbyte {:?}", endb);
			Reader::search(line.iter().copied(), |data| {
				// [{}]
				println!("{:#01x}: {:?}", address, data);
			});

			builder.clear();
			continue;
		}
		if result.is_ignore() {
			continue;
		}
		if result.is_ignore_and_skipdata() {
			break;
		}
	}
}
