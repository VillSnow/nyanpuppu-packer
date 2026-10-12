use std::io::Cursor;

use base64::prelude::*;
use image::buffer::ConvertBuffer;
use ndarray::prelude::*;

const H_DCT_SIZE: usize = 18;
const V_DCT_SIZE: usize = 16;
const T_DCT_SIZE: usize = 4;
const GRAY_IMAGE_SIZE: usize = 32;

pub struct ImageHashData {
    pub hash: String,
    pub grids: Vec<Array2<f32>>,
    pub visualization: Vec<Array2<bool>>,
}

pub fn image_hash(image_file: &[u8]) -> anyhow::Result<String> {
    Ok(image_hash_data(image_file)?.hash)
}

pub fn image_hash_data(image_file: &[u8]) -> anyhow::Result<ImageHashData> {
    let grids = load_frames(image_file)?
        .into_iter()
        .map(frame_to_grid)
        .collect::<Vec<_>>();
    let frames = grids.iter().map(|grid| grid.view()).collect::<Vec<_>>();

    let (hash, visualization) = if frames.len() == 1 {
        let (hash, visualization) = phash2d(&frames[0]);
        (hash, vec![visualization])
    } else {
        let cuboid = ndarray::stack(Axis(0), &frames)?;
        phash3d(&cuboid.view())
    };
    Ok(ImageHashData {
        hash,
        grids,
        visualization,
    })
}

fn load_frames(image_file: &[u8]) -> anyhow::Result<Vec<image::Rgba32FImage>> {
    fn decoder_to_frames<D: image::ImageDecoder>(
        decoder: D,
    ) -> anyhow::Result<Vec<image::Rgba32FImage>> {
        Ok(vec![
            image::DynamicImage::from_decoder(decoder)?.to_rgba32f(),
        ])
    }

    fn animation_decoder_to_frames<'a, D: image::AnimationDecoder<'a>>(
        decoder: D,
    ) -> anyhow::Result<Vec<image::Rgba32FImage>> {
        let frames = decoder.into_frames().collect_frames()?;

        let mut result = Vec::new();
        for frame in frames {
            result.push(frame.buffer().convert());
        }
        if result.is_empty() {
            anyhow::bail!("Empty frames.");
        }

        Ok(result)
    }

    let format = image::guess_format(image_file)?;
    let image_file = Cursor::new(image_file);

    match format {
        image::ImageFormat::Png => {
            let decoder = image::codecs::png::PngDecoder::new(image_file)?;
            if decoder.is_apng()? {
                animation_decoder_to_frames(decoder.apng()?)
            } else {
                decoder_to_frames(decoder)
            }
        }
        image::ImageFormat::Jpeg => {
            decoder_to_frames(image::codecs::jpeg::JpegDecoder::new(image_file)?)
        }
        image::ImageFormat::Gif => {
            animation_decoder_to_frames(image::codecs::gif::GifDecoder::new(image_file)?)
        }
        image::ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(image_file)?;
            if decoder.has_animation() {
                animation_decoder_to_frames(decoder)
            } else {
                decoder_to_frames(decoder)
            }
        }
        image::ImageFormat::Pnm => {
            decoder_to_frames(image::codecs::pnm::PnmDecoder::new(image_file)?)
        }
        image::ImageFormat::Tiff => {
            decoder_to_frames(image::codecs::tiff::TiffDecoder::new(image_file)?)
        }
        image::ImageFormat::Tga => {
            decoder_to_frames(image::codecs::tga::TgaDecoder::new(image_file)?)
        }
        image::ImageFormat::Dds => {
            decoder_to_frames(image::codecs::dds::DdsDecoder::new(image_file)?)
        }
        image::ImageFormat::Bmp => {
            decoder_to_frames(image::codecs::bmp::BmpDecoder::new(image_file)?)
        }
        image::ImageFormat::Ico => {
            decoder_to_frames(image::codecs::ico::IcoDecoder::new(image_file)?)
        }
        image::ImageFormat::Hdr => {
            decoder_to_frames(image::codecs::hdr::HdrDecoder::new(image_file)?)
        }
        image::ImageFormat::OpenExr => {
            decoder_to_frames(image::codecs::openexr::OpenExrDecoder::new(image_file)?)
        }
        image::ImageFormat::Farbfeld => {
            decoder_to_frames(image::codecs::farbfeld::FarbfeldDecoder::new(image_file)?)
        }
        image::ImageFormat::Qoi => {
            decoder_to_frames(image::codecs::qoi::QoiDecoder::new(image_file)?)
        }
        _ => anyhow::bail!("not supported image format"),
    }
}

fn frame_to_grid(image: image::Rgba32FImage) -> Array2<f32> {
    let width = image.width() as usize;
    let height = image.height() as usize;

    let image = Array3::from_shape_vec((height, width, 4), image.into_raw())
        .expect("failed to convert image to array");

    let red = image.slice(s![.., .., 0]);
    let green = image.slice(s![.., .., 1]);
    let blue = image.slice(s![.., .., 2]);
    let alpha = image.slice(s![.., .., 3]);
    let gray = (&red + &green + &blue) / 3.0;

    let gray_black = &gray * &alpha;
    let gray_white = (&gray - 1.0) * &alpha;
    let red_delta_black = (&red - &gray) * &alpha;
    let green_delta_black = (&green - &gray) * &alpha;

    let grid = grid(
        downscale(&gray_black.view()).view(),
        downscale(&red_delta_black.view()).view(),
        downscale(&green_delta_black.view()).view(),
        downscale(&gray_white.view()).view(),
    );

    grid
}

fn downscale(image: &ArrayView2<f32>) -> Array2<f32> {
    let a = image
        .iter()
        .copied()
        .min_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();

    let b = image
        .iter()
        .copied()
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap();

    let image = (image - a) / (b - a + 1e-6);

    let (height, width) = image.dim();
    let image = image::ImageBuffer::<image::Luma<f32>, Vec<f32>>::from_raw(
        width as u32,
        height as u32,
        image.flatten().to_vec(),
    )
    .expect("failed to convert array to image");

    let image = image::imageops::resize(
        &image,
        GRAY_IMAGE_SIZE as u32,
        GRAY_IMAGE_SIZE as u32,
        image::imageops::FilterType::Triangle,
    );

    let image = Array2::from_shape_vec((GRAY_IMAGE_SIZE, GRAY_IMAGE_SIZE), image.into_raw())
        .expect("failed to convert image to array");

    let image = image * (b - a + 1e-6) + a;

    image
}

fn grid(
    a: ArrayView2<f32>,
    b: ArrayView2<f32>,
    c: ArrayView2<f32>,
    d: ArrayView2<f32>,
) -> Array2<f32> {
    use ndarray::concatenate;

    let a = a.slice(s![.., ..;-1]);
    let c = c.slice(s![.., ..;-1]);

    let a = a.slice(s![..;-1, ..]);
    let b = b.slice(s![..;-1, ..]);

    concatenate![
        Axis(0),
        concatenate![Axis(1), a, b],
        concatenate![Axis(1), c, d],
    ]
}

fn phash2d(image: &ArrayView2<f32>) -> (String, Array2<bool>) {
    let kmap = dct2d(&image.view());
    let feature = kmap
        .slice(s![..V_DCT_SIZE, ..H_DCT_SIZE])
        .flatten()
        .into_iter()
        .collect::<Vec<_>>();
    let bits = hash_bits(&feature);
    let visualization = Array2::from_shape_vec((V_DCT_SIZE, H_DCT_SIZE), bits.clone())
        .expect("static hash visualization shape is valid");
    (encode_hash_bits(&bits), visualization)
}

fn phash3d(image: &ArrayView3<f32>) -> (String, Vec<Array2<bool>>) {
    let kmap = dct3d(&image.view());
    let dur_size = kmap.shape()[0].min(T_DCT_SIZE);
    let feature = kmap
        .slice(s![..dur_size, ..V_DCT_SIZE, ..H_DCT_SIZE])
        .flatten()
        .into_iter()
        .collect::<Vec<_>>();
    let bits = hash_bits(&feature);
    let visualization = bits
        .chunks_exact(V_DCT_SIZE * H_DCT_SIZE)
        .map(|panel| {
            Array2::from_shape_vec((V_DCT_SIZE, H_DCT_SIZE), panel.to_vec())
                .expect("animated hash visualization panel shape is valid")
        })
        .collect();
    (encode_hash_bits(&bits), visualization)
}

fn hash_bits(feature: &[f32]) -> Vec<bool> {
    let mean = iter_mean(feature[1..].iter().copied());
    feature.iter().map(|value| *value > mean).collect()
}

fn encode_hash_bits(bits: &[bool]) -> String {
    let bytes = bits
        .chunks(8)
        .map(|chunk| {
            chunk
                .into_iter()
                .copied()
                .enumerate()
                .fold(0u8, |acc, (i, bit)| if bit { acc | 1 << i } else { acc })
        })
        .collect::<Vec<_>>();
    BASE64_STANDARD.encode(bytes)
}

fn dct2d(image: &ArrayView2<f32>) -> ndarray::Array2<f32> {
    use ndrustfft::{DctHandler, nddct2};
    let (height, width) = image.dim();

    let mut h_result = Array2::<f32>::zeros((height, width));
    let mut fft_handler = DctHandler::<f32>::new(width);
    nddct2(&image, &mut h_result.view_mut(), &mut fft_handler, 1);

    let mut hv_result = Array2::<f32>::zeros((height, width));
    let mut fft_handler = DctHandler::<f32>::new(height);
    nddct2(&h_result, &mut hv_result.view_mut(), &mut fft_handler, 0);

    hv_result
}

fn dct3d(image: &ArrayView3<f32>) -> ndarray::Array3<f32> {
    use ndrustfft::{DctHandler, nddct2};
    let (dur, height, width) = image.dim();

    let mut h_result = Array3::<f32>::zeros((dur, height, width));
    let mut fft_handler = DctHandler::<f32>::new(width);
    nddct2(&image, &mut h_result.view_mut(), &mut fft_handler, 2);

    let mut hv_result = Array3::<f32>::zeros((dur, height, width));
    let mut fft_handler = DctHandler::<f32>::new(height);
    nddct2(&h_result, &mut hv_result.view_mut(), &mut fft_handler, 1);

    let mut hvt_result = Array3::<f32>::zeros((dur, height, width));
    let mut fft_handler = DctHandler::<f32>::new(dur);
    nddct2(&hv_result, &mut hvt_result.view_mut(), &mut fft_handler, 0);

    hvt_result
}

fn iter_mean(xs: impl IntoIterator<Item = f32>) -> f32 {
    let mut xs = xs.into_iter();

    let mut s = 0.0;
    let mut n = 0;
    while let Some(x) = xs.next() {
        s += x;
        n += 1
    }

    s / n as f32
}

pub fn distance(lhs: &str, rhs: &str) -> u32 {
    let lhs = BASE64_STANDARD
        .decode(lhs)
        .expect("failed to decode hash value");
    let rhs = BASE64_STANDARD
        .decode(rhs)
        .expect("failed to decode hash value");
    if lhs.len() != rhs.len() {
        return u32::MAX;
    }
    lhs.into_iter()
        .zip(rhs)
        .map(|(l, r)| (l ^ r).count_ones())
        .sum()
}

fn _save_ndarray_as_image(array: &ArrayView2<f32>, path: &str) -> Result<(), image::ImageError> {
    let (height, width) = array.dim();

    let mut img = image::GrayImage::new(width as u32, height as u32);

    for ((y, x), pixel) in array.indexed_iter() {
        let pixel = (128.0 + pixel * 128.0).floor().clamp(0.0, 255.0) as u8;
        img.put_pixel(x as u32, y as u32, image::Luma([pixel]));
    }

    img.save(path)
}
