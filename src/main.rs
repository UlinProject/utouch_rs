use crate::config::DEFAULT_I2C_BUS;
use crate::config::DISPLAY_HEIGHT;
use crate::config::DISPLAY_WIDTH;
use crate::config::I2C_ADDR;
use crate::config::I2C_NUM_BUS0;
use crate::config::NEEDS_COORDINATE_INVERSION;
use crate::config::RPPAL_INT_PIN;
use crate::config::RPPAL_RESPIN;
use crate::model::BuildReader;
use crate::model::Reader;
use enclose::enc;
use log::error;
use log::info;
use log::trace;
use rand::Rng;
use rand::rng;
use rppal::gpio::Gpio;
use rppal::i2c::I2c;
use std::env::set_var;
use std::env::var_os;
use std::error::Error;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fs::read_dir;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::Barrier;
use std::sync::mpsc::channel;
use std::thread::sleep;
use std::thread::spawn;
use std::time::Duration;
use tfc::Context;
use tfc::MouseButton;
use tfc::MouseContext;
use uinput::Device;
use uinput::event::Absolute::Multi;
use uinput::event::Controller::Digi;
use uinput::event::Event::Absolute;
use uinput::event::Event::Controller;
use uinput::event::absolute::Multi::{PositionX, PositionY, Slot, TrackingId};
use uinput::event::controller::Digi::Touch;

mod config;
mod core;
mod model;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CTypeTransformCoordinates {
	Ver01 = 1,
	Ver02 = 2,
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum CIdentDevice {
	X11Mouse(CTypeTransformCoordinates),
	UInput,
}

pub enum InputDevice {
	X11Mouse {
		context: Context,
		transform: CTypeTransformCoordinates,

		cx: u16,
		cy: u16,

		is_add_click: bool,
	},
	UInput {
		device: Device,
		a_slot: usize,

		fingers: [Option<i8>; 12],
	},
}

impl InputDevice {
	pub fn new(c_ident_device: CIdentDevice) -> Result<Self, Box<dyn Error>> {
		match c_ident_device {
			CIdentDevice::X11Mouse(transform) => {
				let context = Context::new()?;

				Ok(Self::X11Mouse {
					context,
					transform,

					cx: 0,
					cy: 0,

					is_add_click: false,
				})
			}
			CIdentDevice::UInput => {
				let mut rng = rng();
				let device = uinput::default()?
					// Device name
					.name(format!("utouch_rs_{}", rng.random::<u64>()))?
					.event(Absolute(Multi(PositionX)))?
					.min(0)
					.max(DISPLAY_WIDTH as _)
					.event(Absolute(Multi(PositionY)))?
					.min(0)
					.max(DISPLAY_HEIGHT as _)
					.event(Absolute(Multi(Slot)))?
					//.min(0).max(12)
					.event(Absolute(Multi(TrackingId)))?
					.min(0)
					.max(12)
					.event(Controller(Digi(Touch)))?
					.create()?;

				Ok(Self::UInput {
					device,
					fingers: Default::default(),
					a_slot: 0,
				})
			}
		}
	}

	pub fn initialize_event_aggregator(&mut self) {
		/*match self {
			Self::X11Mouse { context, transform, cx, cy, is_add_click } => {},
			Self::UInput { device, fingers, a_slot } => {

			},
		}*/
	}

	pub fn drop_event(&mut self) {
		match self {
			Self::X11Mouse { .. /*context, transform, cx, cy, is_add_click*/ } => {},
			Self::UInput { device: _, fingers: _, a_slot } => {
				*a_slot += 1;
			},
		}
	}

	pub fn push_move(
		&mut self,
		address: u16,
		id: i8,
		mut x: u16,
		mut y: u16,
	) -> Result<(), Box<dyn Error>> {
		match self {
			Self::X11Mouse {
				context,
				transform,
				cx,
				cy,
				is_add_click,
			} => {
				if id > 1 {
					trace!("#{:#01x} ignore", address);
					return Ok(());
				}

				if x != *cx || y != *cy {
					*cx = x;
					*cy = y;

					/* INVERS */
					if NEEDS_COORDINATE_INVERSION {
						std::mem::swap(&mut y, &mut x);
					}

					let m_x;
					let m_y;

					match transform {
						CTypeTransformCoordinates::Ver01 => {
							m_x = x as i32;
							m_y = (DISPLAY_HEIGHT as i32) - (y as i32);
						}
						CTypeTransformCoordinates::Ver02 => {
							#[inline]
							const fn map(
								x: i32,
								in_min: i32,
								in_max: i32,
								out_min: i32,
								out_max: i32,
							) -> i32 {
								(x - in_min) * (out_max - out_min) / (in_max - in_min) + out_min
							}

							m_x = map(x as i32, 0, DISPLAY_WIDTH as _, 0, DISPLAY_HEIGHT as _);
							m_y = map(
								y as i32,
								DISPLAY_HEIGHT as _,
								0,
								0,
								(DISPLAY_HEIGHT as i32) / 2,
							);
						}
					}

					trace!(
						"#[x11_move]: [x: {}, y: {}] => [m_x: {}, m_y: {}]",
						x, y, m_x, m_y
					);
					if let Err(e) = context.mouse_move_abs(m_x, m_y) {
						error!("mouse, err: {:?}", e);
					}

					*is_add_click = true;
				}

				Ok(())
			}
			Self::UInput {
				device,
				fingers,
				a_slot,
			} => {
				trace!("#[touch_move]: id: {}, x: {}, y: {}", id, x, y);
				let mut x: i32 = x as _;
				let mut y: i32 = y as _;

				/* INVERS */
				std::mem::swap(&mut y, &mut x);
				y = (DISPLAY_HEIGHT as i32) - y;

				let mut is_exists = false;
				for a_id in fingers.iter().flatten() {
					if a_id == &id {
						is_exists = true;
						break;
					}
				}

				device.position(&Slot, *a_slot as _)?;
				if !is_exists {
					device.press(&Touch)?;
					for ref_finger in fingers.iter_mut() {
						if ref_finger.is_none() {
							*ref_finger = Some(id);
							break;
						}
					}
				}
				device.position(&TrackingId, id as _)?;
				device.position(&PositionX, x)?;
				device.position(&PositionY, y)?;
				device.synchronize()?;

				Ok(())
			}
		}
	}

	pub fn free_time(&mut self) -> Result<(), Box<dyn Error>> {
		match self {
			Self::X11Mouse {
				context,
				transform: _,
				cx: _,
				cy: _,
				is_add_click,
			} if is_add_click == &true => {
				*is_add_click = false;
				trace!("#[x11_click]");

				context.mouse_click(MouseButton::Left)?;
				Ok(())
			}
			Self::X11Mouse { .. } => Ok(()),
			Self::UInput {
				device,
				fingers,
				a_slot,
			} => {
				for ref_finger in fingers.iter_mut() {
					if let Some(id) = ref_finger {
						device.position(&Slot, *a_slot as _)?;
						device.position(&TrackingId, *id as _)?;
						device.release(&Touch)?;
						device.synchronize()?;

						*ref_finger = None;
					}
				}

				while *a_slot != 0 {
					device.position(&Slot, *a_slot as _)?;
					device.position(&TrackingId, -1)?;
					device.synchronize()?;

					*a_slot -= 1;
				}

				// a_slot 0
				device.position(&Slot, *a_slot as _)?;
				device.position(&TrackingId, -1)?;
				device.synchronize()?;

				*a_slot = 0;

				Ok(())
			}
		}
	}

	pub fn init_press(&mut self) -> Result<(), Box<dyn Error>> {
		match self {
			Self::X11Mouse { .. /*context, transform, cx, cy, is_add_click*/ } => {
				Ok(())
			},
			Self::UInput { .. /*device, fingers, a_slot*/ } => {
				Ok(())
			},
		}
	}
}

fn main() -> Result<(), Box<dyn Error>> {
	env_logger::try_init()?;
	info!("utouch_rs: ");

	let mut owned_a = OsString::new();
	let c_ident_device = match var_os("CDEVICE").map(|a| {
		owned_a = a;
		owned_a.as_os_str()
	}) {
		Some(a) if a == osstr!("X11_MOUSE") || a == osstr!("X11") || a == osstr!("MOUSE") => {
			let c_type_transform_coordinates = match var_os("CTYPE") {
				Some(a) if a == osstr!("01") || a == osstr!("1") => {
					CTypeTransformCoordinates::Ver01
				}
				Some(a) if a == osstr!("02") || a == osstr!("2") => {
					CTypeTransformCoordinates::Ver02
				}

				_ => CTypeTransformCoordinates::Ver01,
			};
			info!("ctype: {:?}", c_type_transform_coordinates);

			match var_os("DISPLAY") {
				Some(a) => {
					info!("DISPLAY={:?}", a);
				}
				None => {
					unsafe { set_var("DISPLAY", ":0") };
					info!("DISPLAY=:0");
				}
			};
			match var_os("XAUTHORITY") {
				Some(a) => {
					info!("XAUTHORITY={:?}", a);
				}
				None => {
					let mut is_exists = false;
					let paths = [
						|| Path::new("/tmp/"),
						|| Path::new("/home/alarm/"), // TODO get current username
						|| Path::new("/root/"),
					];
					'search_xauth: for make_path in paths {
						let tmp_path = Path::new((make_path)());
						info!("auto search XAUTHORITY, in: {:?}", tmp_path);

						match read_dir(tmp_path) {
							Ok(entry) => {
								for entry in entry.flatten() {
									let path = entry.path();
									if path.is_file() {
										if let Some(filename) = path.file_name() {
											// todo, osstr, linux
											if filename.as_bytes().starts_with(b"xauth_")
												|| filename == osstr!(".Xauthority")
											{
												info!("XAUTHORITY={:?}", path);
												unsafe { set_var("XAUTHORITY", path) };

												is_exists = true;
												break 'search_xauth;
											}
										}
									}
								}
							}
							Err(e) => {
								error!("auto search XAUTHORITY, in: {:?}, err: {:?}", tmp_path, e);
							}
						}
					}
					if !is_exists {
						info!("Unknown XAUTHORITY.");
					}
				}
			};

			CIdentDevice::X11Mouse(c_type_transform_coordinates)
		}
		Some(a) if a == osstr!("UINPUT") || a == osstr!("LINUX") || a == osstr!("TOUCH") => {
			CIdentDevice::UInput
		}

		_ => CIdentDevice::UInput,
	};

	info!("cdevice: {:?}", c_ident_device);
	info!("attention, Interrupt int is not serviced.");
	info!("");
	let gpio = Gpio::new()?;

	{
		// RESET
		info!("#[pin, {:?}] init, output", RPPAL_RESPIN);
		let mut reset_pin = gpio.get(RPPAL_RESPIN)?.into_output();

		info!("#[pin, {:?}] reset...", RPPAL_RESPIN);
		reset_pin.set_low();
		sleep(Duration::from_millis(1000_u64));
		reset_pin.set_high();
		sleep(Duration::from_millis(5_u64));
	}

	// INTERRUPT TODO
	info!("#[pin, {:?}] init, input", RPPAL_INT_PIN);
	let mut int_pin = gpio.get(RPPAL_INT_PIN)?.into_input();
	//let _ = int_pin.clear_interrupt();
	//let _ = int_pin.clear_async_interrupt();
	//println!("#[pin, {:?}] init interrupt", RPPAL_INT_PIN);
	//int_pin.set_interrupt(Trigger::Both)?;

	// I2C
	info!("#[i2c] init bus, num: auto");

	let mut i2c = I2C_NUM_BUS0
		.map_or_else(I2c::new, I2c::with_bus)
		.or_else(|e| {
			error!("#[i2c] init bus, {}", e);
			info!("modprobe i2c_dev;");
			let _e = Command::new("modprobe").arg("i2c_dev").output();

			info!("#[i2c] init bus, num: auto");

			I2c::new().or_else(|e| {
				println!("#[i2c] init bus, err: {:?}", e);
				println!("#[i2c] init bus, num: 1");

				I2c::with_bus(DEFAULT_I2C_BUS)
			})
		})?;

	info!(
		"#[i2c, {:#01x}, {:?}hz, bus_num: {}] init addr",
		I2C_ADDR,
		i2c.clock_speed(),
		i2c.bus()
	);

	i2c.set_slave_address(I2C_ADDR)?;
	info!("#[i2c, {:#01x}] prepare", I2C_ADDR);

	let mut builder = BuildReader::empty();
	let mut i2carray = vec![0u8; 60];

	let (tx, rx) = channel::<()>();
	let wait_init_thread = Arc::new(Barrier::new(1 + 1));
	spawn(enc!((wait_init_thread) move || {
		{
			wait_init_thread.wait();
			drop(wait_init_thread);
		}

		loop {
			let int = int_pin.poll_interrupt(false, Some(Duration::from_millis(150))); // INTERRUPT TODO
			if let Err(ref e) = int {
				error!("Err, {:?}", e);
			}
			if tx.send(()).is_err() {
				break; // END CTHREAD
			}
		}
	}));
	{
		wait_init_thread.wait();
		drop(wait_init_thread);
	}

	// RES+INTERRUPT+DECODER
	let mut is_addition_interrupt = false;
	info!("#[cdevice] init...");
	let mut input_device = InputDevice::new(c_ident_device)?;
	sleep(Duration::from_millis(300));
	info!("#[cdevice] loop:");
	loop {
		if !is_addition_interrupt {
			if rx.recv().is_err() {
				// WAIT INTERRUPT
				break;
			}
		} else {
			is_addition_interrupt = false;
			// ADDITION INTERRUPT
		}

		trace!("#[i2c, {:#01x}] read...", I2C_ADDR);
		let size = i2c.read(i2carray.as_mut_slice())?;
		trace!("#[i2c, {:#01x}] ok, size: {}", I2C_ADDR, size);

		let data = match i2carray.get_mut(..size) {
			Some(a) => a,
			None => {
				error!(
					"#[i2c, {:#01x}] invalid get slice buff (..{})",
					I2C_ADDR, size
				);
				&mut []
			}
		};
		if data.is_empty() {
			continue;
		}

		trace!("#[i2c, {:#01x}] data: {:?}", I2C_ADDR, data);
		for a in data.iter() {
			let result = builder.write(*a);

			if result.is_end_line() {
				is_addition_interrupt = false;
				let (address, line, endb) = builder.get_line();

				/*if line.len() > 0 {
					println!("#line {:?}, is_addition_interrupt: {:?}, nbyte: {:?} == {:?}", line, is_addition_interrupt, nbyte, data.len());
				}*/
				if endb != 0 {
					trace!("#endbyte {:?}", endb);
				}
				input_device.initialize_event_aggregator();
				let mut is_evented = false;
				Reader::search(line.iter().copied(), |data| {
					is_addition_interrupt = true;
					is_evented = true;

					//println!("#endbyte {:?}", endb);
					if endb == 0 {
						let id: i8 = (data[1] as i8) - 16;

						let x: u16 = u16::from_le_bytes([data[2], data[3] & 0b0000_1111]);
						let y: u16 = u16::from_le_bytes([data[4], data[3] & 0b1111_0000]) << 4;

						let _e = input_device.push_move(address, id, x, y);
					} else if endb == 1 {
						//let _e = cdevice.free_time();
					}
					input_device.drop_event();
				});
				if !is_evented {
					let _e = input_device.free_time();
				}

				builder.clear();
				continue;
			} else if result.is_ignore() {
				continue;
			} else if result.is_continue() {
				is_addition_interrupt = true;
				continue;
			} else if result.is_ignore_and_skipdata() {
				is_addition_interrupt = false;
				break;
			}
		}

		// FLUSH OLD DATA
		for a_write in data.iter_mut() {
			*a_write = 0;
		}
	}

	Ok(())
}
