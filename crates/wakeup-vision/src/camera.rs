use anyhow::{Context, Result, bail};
use image::RgbImage;
use nokhwa::Camera as NokhwaCamera;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
};

pub struct Camera {
    inner: NokhwaCamera,
}

/// Asks for camera permission where the OS requires it (macOS) and blocks
/// until the user answers.
pub fn request_permission() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        nokhwa::nokhwa_initialize(move |granted| {
            let _ = tx.send(granted);
        });
        if !rx.recv().unwrap_or(false) {
            bail!(
                "camera permission denied. Allow it in System Settings > Privacy & Security > Camera"
            );
        }
    }
    Ok(())
}

impl Camera {
    /// Opens camera `index` as close to `width`x`height` as it supports.
    /// Falls back through pixel formats since built-in Mac cameras and USB
    /// webcams expose different ones.
    pub fn open(index: u32, width: u32, height: u32) -> Result<Self> {
        let res = Resolution::new(width, height);
        let attempts = [
            RequestedFormatType::Closest(CameraFormat::new(res, FrameFormat::MJPEG, 30)),
            RequestedFormatType::Closest(CameraFormat::new(res, FrameFormat::YUYV, 30)),
            RequestedFormatType::Closest(CameraFormat::new(res, FrameFormat::NV12, 30)),
            RequestedFormatType::None,
        ];
        let mut last_err = None;
        for kind in attempts {
            let format = RequestedFormat::new::<RgbFormat>(kind);
            match NokhwaCamera::new(CameraIndex::Index(index), format) {
                Ok(mut inner) => {
                    inner.open_stream().context("starting camera stream")?;
                    return Ok(Self { inner });
                }
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.map_or_else(|| anyhow::anyhow!("no camera formats tried"), Into::into))
            .with_context(|| format!("opening camera {index}"))
    }

    pub fn resolution(&self) -> (u32, u32) {
        let r = self.inner.resolution();
        (r.width(), r.height())
    }

    pub fn frame(&mut self) -> Result<RgbImage> {
        let frame = self.inner.frame().context("reading frame")?;
        let decoded = frame
            .decode_image::<RgbFormat>()
            .context("decoding frame")?;
        let (w, h) = (decoded.width(), decoded.height());
        match RgbImage::from_raw(w, h, decoded.into_raw()) {
            Some(img) => Ok(img),
            None => bail!("camera returned a malformed {w}x{h} frame"),
        }
    }
}

impl Drop for Camera {
    fn drop(&mut self) {
        let _ = self.inner.stop_stream();
    }
}
