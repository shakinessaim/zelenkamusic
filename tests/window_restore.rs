// Copyright © ZelenkaMusic contributors
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::num::NonZeroU32;
use i_slint_backend_winit::restore_test_presentation_damage as presentation_damage;
use slint::platform::{software_renderer::RepaintBufferType, PlatformError};
use slint::ComponentHandle;
use std::rc::Rc;

slint::include_modules!();

#[test]
fn retained_image_restores_exposed_edges_and_unchanged_scene() {
    use slint::platform::{software_renderer::MinimalSoftwareWindow, Platform, WindowAdapter};

    struct Headless(Rc<MinimalSoftwareWindow>);
    impl Platform for Headless {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            Ok(self.0.clone())
        }
    }
    let adapter = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(adapter.clone()))).unwrap();
    let scene = AppWindow::new().unwrap();
    scene.set_has_track(true);
    scene.show().unwrap();
    let mut empty_redraws = 0;
    let mut partial_redraws = 0;
    for (width, height) in [(1120, 820), (940, 760), (1280, 900)] {
        scene
            .window()
            .set_size(slint::PhysicalSize::new(width, height));
        let mut retained = vec![slint::Rgb8Pixel::default(); (width * height) as usize];
        for animated in [false, true] {
            scene.set_snow(animated);
            scene.window().request_redraw();
            adapter.draw_if_needed(|renderer| {
                renderer.set_repaint_buffer_type(RepaintBufferType::NewBuffer);
                renderer.render(&mut retained, width as usize);
                renderer.set_repaint_buffer_type(RepaintBufferType::ReusedBuffer);
            });
            for cycle in 0..8 {
                if animated {
                    scene.set_snow_phase(cycle as f32 * 7.3 + 1.0);
                    scene.set_progress(cycle as f32 / 8.0);
                }
                scene.window().request_redraw();
                let mut region = None;
                adapter.draw_if_needed(|renderer| {
                    region = Some(renderer.render(&mut retained, width as usize));
                });
                let region = region.expect("Requested redraw must be delivered");
                let dirty = region.bounding_box_size();
                if dirty.width == 0 || dirty.height == 0 {
                    empty_redraws += 1;
                } else if dirty.width < width && dirty.height < height {
                    partial_redraws += 1;
                }
                // The DIB remains valid while the displayed client surface is lost.
                // This models OS exposure, independently of Slint scene invalidation.
                let mut exposed = vec![slint::Rgb8Pixel::default(); retained.len()];
                let rect = presentation_damage(
                    NonZeroU32::new(width).unwrap(),
                    NonZeroU32::new(height).unwrap(),
                    &region,
                )
                .expect("An unchanged scene must still be presented after OS exposure");
                for y in rect.y..rect.y + rect.height.get() {
                    let start = (y * width + rect.x) as usize;
                    let end = start + rect.width.get() as usize;
                    exposed[start..end].copy_from_slice(&retained[start..end]);
                }
                assert!(exposed == retained, "Missing pixels after restore: animated={animated}, cycle={cycle}, size={width}x{height}");
            }
        }
    }
    assert!(
        empty_redraws >= 18,
        "Must cover an entirely unchanged UI: {empty_redraws}"
    );
    assert!(
        partial_redraws >= 18,
        "Must retain incremental rendering during animation: {partial_redraws}"
    );
    println!("Restored 48 surfaces in 3 sizes: {empty_redraws} unchanged, {partial_redraws} partial redraws");
    scene.hide().unwrap();
}

#[test]
fn native_gdi_restores_unchanged_surface() {
    use std::ffi::c_void;
    type Handle = *mut c_void;
    #[repr(C)]
    struct BitmapInfo {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bit_count: u16,
        compression: u32,
        image_size: u32,
        x_ppm: i32,
        y_ppm: i32,
        colors_used: u32,
        colors_important: u32,
        colors: [u32; 3],
    }
    #[link(name = "gdi32")]
    extern "system" {
        fn CreateCompatibleDC(dc: Handle) -> Handle;
        fn DeleteDC(dc: Handle) -> i32;
        fn CreateDIBSection(
            dc: Handle,
            info: *const BitmapInfo,
            usage: u32,
            bits: *mut *mut c_void,
            section: Handle,
            offset: u32,
        ) -> Handle;
        fn SelectObject(dc: Handle, object: Handle) -> Handle;
        fn DeleteObject(object: Handle) -> i32;
        fn BitBlt(
            dst: Handle,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            src: Handle,
            sx: i32,
            sy: i32,
            operation: u32,
        ) -> i32;
        fn GdiFlush() -> i32;
    }
    struct Dib {
        dc: Handle,
        bitmap: Handle,
        original: Handle,
        bits: *mut u32,
    }
    impl Dib {
        fn new() -> Self {
            let info = BitmapInfo {
                size: 40,
                width: 128,
                height: -96,
                planes: 1,
                bit_count: 32,
                compression: 0,
                image_size: 0,
                x_ppm: 0,
                y_ppm: 0,
                colors_used: 0,
                colors_important: 0,
                colors: [0; 3],
            };
            unsafe {
                let dc = CreateCompatibleDC(std::ptr::null_mut());
                assert!(!dc.is_null());
                let mut bits = std::ptr::null_mut();
                let bitmap = CreateDIBSection(dc, &info, 0, &mut bits, std::ptr::null_mut(), 0);
                assert!(!bitmap.is_null());
                let original = SelectObject(dc, bitmap);
                Self {
                    dc,
                    bitmap,
                    original,
                    bits: bits.cast(),
                }
            }
        }
    }
    impl Drop for Dib {
        fn drop(&mut self) {
            unsafe {
                SelectObject(self.dc, self.original);
                DeleteObject(self.bitmap);
                DeleteDC(self.dc);
            }
        }
    }
    let retained = Dib::new();
    let displayed = Dib::new();
    let source = unsafe { std::slice::from_raw_parts_mut(retained.bits, 128 * 96) };
    // Distinct colors at every position expose incomplete copies, including corners.
    for (index, pixel) in source.iter_mut().enumerate() {
        *pixel = 0x00101512 + index as u32;
    }
    for cycle in 0..12 {
        unsafe {
            std::slice::from_raw_parts_mut(displayed.bits, source.len()).fill(0);
        }
        let damage = presentation_damage(
            NonZeroU32::new(128).unwrap(),
            NonZeroU32::new(96).unwrap(),
            &slint::platform::software_renderer::PhysicalRegion::default(),
        )
        .expect("Native presentation must run even without scene changes");
        unsafe {
            assert_ne!(
                BitBlt(
                    displayed.dc,
                    damage.x as i32,
                    damage.y as i32,
                    damage.width.get() as i32,
                    damage.height.get() as i32,
                    retained.dc,
                    damage.x as i32,
                    damage.y as i32,
                    0x00CC0020
                ),
                0
            );
            assert_ne!(GdiFlush(), 0);
        }
        let target = unsafe { std::slice::from_raw_parts(displayed.bits, source.len()) };
        assert!(
            target == source,
            "Native GDI lost pixels after restore {cycle}"
        );
    }
    println!("Native Win32 BitBlt restored all pixels in 12 cycles");
}
