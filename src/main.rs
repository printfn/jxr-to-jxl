use eyre::bail;
use half::f16;
use jpegxl_rs::{
	encode::{ColorEncoding, EncoderFrame},
	encoder_builder,
};
use jpegxl_sys::common::types::JxlEndianness;
use jpegxr::{ImageDecode, PixelInfo};
use std::{env, fs};

fn main() -> eyre::Result<()> {
	let args = env::args_os().collect::<Vec<_>>();
	if args.len() != 2 {
		bail!("Usage: jxr-to-jxl <input.jxr>");
	}
	let input = &args[1];
	let mut output = input.to_os_string();
	if let Some(s) = output.to_str() {
		output = s.to_lowercase().trim_end_matches(".jxr").into();
	}
	output.push(".jxl");
	println!("Reading {} as JXR", input.to_string_lossy());
	let input = fs::File::open(input)?;
	let mut decoder = ImageDecode::with_reader(input)?;

	let (width, height) = decoder.get_size()?;
	let width = usize::try_from(width)?;
	let height = usize::try_from(height)?;
	println!("Image size: {width} by {height}");
	let info = PixelInfo::from_format(decoder.get_pixel_format()?);
	println!("Bit depth: {:?}", info.bit_depth());
	println!("{} bits per pixel", info.bits_per_pixel());
	println!("{} channels", info.channels());
	println!("Color format: {:?}", info.color_format());
	println!("Has alpha: {}", info.has_alpha());
	println!("Premultiplied alpha: {}", info.premultiplied_alpha());
	println!(
		"Photometric interpretation: {:?}",
		info.photometric_interpretation()
	);
	println!("Samples per pixel: {}", info.samples_per_pixel());
	println!("BGR: {}", info.bgr());
	let stride = width * info.bits_per_pixel() / 8;
	let size = stride * height;

	let mut buffer = vec![0; size];
	decoder.copy_all(&mut buffer, stride)?;
	println!("{} bytes in buffer", buffer.len());
	println!(
		"buffer contains {} bytes per pixel",
		buffer.len() / width / height
	);
	println!("{:2x?}", &buffer[..10]);

	let mut float_buffer = Vec::<f16>::with_capacity(size);
	for i in 0..width * height {
		float_buffer
			.push(f16::from_le_bytes([buffer[i * 8], buffer[i * 8 + 1]]) / f16::from_f64(3.));
		float_buffer
			.push(f16::from_le_bytes([buffer[i * 8 + 2], buffer[i * 8 + 3]]) / f16::from_f64(3.));
		float_buffer
			.push(f16::from_le_bytes([buffer[i * 8 + 4], buffer[i * 8 + 5]]) / f16::from_f64(3.));
	}
	println!("{:2x?}", &float_buffer[..10]);
	println!(
		"Maximum: {}",
		float_buffer
			.iter()
			.copied()
			.fold(f16::NEG_INFINITY, f16::max)
	);

	let runner = jpegxl_rs::ThreadsRunner::default();
	let mut encoder = encoder_builder()
		.lossless(true)
		.uses_original_profile(true)
		.speed(jpegxl_rs::encode::EncoderSpeed::Cheetah)
		.parallel_runner(&runner)
		.use_container(true)
		.build()?;
	encoder.color_encoding = ColorEncoding::LinearSrgb;
	let frame = EncoderFrame::new(&float_buffer[..])
		.num_channels(3)
		.endianness(JxlEndianness::Little);
	let result = encoder.encode_frame::<f16, f16>(&frame, width.try_into()?, height.try_into()?)?;
	println!("Writing JXL to {}", output.to_string_lossy());
	fs::write(output, &*result)?;
	Ok(())
}
