// GPIO4 pin number for interrupt signal. TODO: Verify correct pin.
pub const RPPAL_INT_PIN: u8 = 4;
// GPIO17 pin number for response signal. TODO: Verify correct pin.
pub const RPPAL_RESPIN: u8 = 17;

pub const I2C_ADDR: u16 = 0x4B; // I2C address of the device.
pub const INVALID_BYTE: u8 = 0x5A; // Value indicating an invalid or uninitialized byte.

pub const DISPLAY_WIDTH: usize = 1520; // Display width in pixels.
pub const DISPLAY_HEIGHT: usize = 720; // Display height in pixels.

// If None, the library will attempt to automatically detect the correct I2C bus.
pub const I2C_NUM_BUS0: Option<u8> = None;
// If automatic bus is not available then use this bus
pub const DEFAULT_I2C_BUS: u8 = 1;

// Flag indicating that the coordinate should be inverted.
pub const NEEDS_COORDINATE_INVERSION: bool = true;
