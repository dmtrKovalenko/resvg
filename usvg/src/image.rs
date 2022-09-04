// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::sync::Arc;
use svgtypes::Length;

use crate::geom::{Rect, Transform, ViewBox};
use crate::svgtree::{self, AId};
use crate::{
    converter, ImageRendering, Node, NodeExt, NodeKind, OptionLog, OptionsRef, Visibility,
};

/// Use this struct to preload, decode and cache images for the upcoming rendering.
#[derive(Debug)]
pub struct PreloadedImageData {
    /// The decoded image data. Make sure that if you submit the data directly it must be blended for semi transparent colors.
    pub data: Vec<u8>,
    /// The width of image in pixels
    pub width: u32,
    /// The height of image in pixels
    pub height: u32,
}

impl PreloadedImageData {
    /// Creates a new `PreloadedImageData` from the given rgba8 buffer and blends all the semi transparent colors.
    pub fn new(width: u32, height: u32, rgba_data: Vec<u8>) -> Arc<Self> {
        let mut data = vec![0; rgba_data.len()];

        for i in (0..rgba_data.len()).step_by(4) {
            let r = rgba_data[i];
            let g = rgba_data[i + 1];
            let b = rgba_data[i + 2];
            let a = rgba_data[i + 3];

            let alpha = a as f32 / 255.0;

            data[i + 0] = (r as f32 * alpha + 0.5) as u8;
            data[i + 1] = (g as f32 * alpha + 0.5) as u8;
            data[i + 2] = (b as f32 * alpha + 0.5) as u8;
            data[i + 3] = a;
        }

        Arc::new(Self {
            data,
            width,
            height,
        })
    }
}

/// A raster image element.
///
/// `image` element in SVG.
#[derive(Clone, Debug)]
pub struct Image {
    /// Element's ID.
    ///
    /// Taken from the SVG itself.
    /// Isn't automatically generated.
    /// Can be empty.
    pub id: String,

    /// Element transform.
    pub transform: Transform,

    /// Element visibility.
    pub visibility: Visibility,

    /// An image rectangle in which it should be fit.
    ///
    /// Combination of the `x`, `y`, `width`, `height` and `preserveAspectRatio`
    /// attributes.
    pub view_box: ViewBox,

    /// Rendering mode.
    ///
    /// `image-rendering` in SVG.
    pub rendering_mode: ImageRendering,

    /// Image data.
    pub data: Arc<PreloadedImageData>,
}

pub(crate) fn convert(
    node: svgtree::Node,
    state: &converter::State,
    parent: &mut Node,
) -> Option<()> {
    let visibility = node.find_attribute(AId::Visibility).unwrap_or_default();
    let rendering_mode = node
        .find_attribute(AId::ImageRendering)
        .unwrap_or(state.opt.image_rendering);

    let rect = Rect::new(
        node.convert_user_length(AId::X, state, Length::zero()),
        node.convert_user_length(AId::Y, state, Length::zero()),
        node.convert_user_length(AId::Width, state, Length::zero()),
        node.convert_user_length(AId::Height, state, Length::zero()),
    );
    let rect = rect.log_none(|| log::warn!("Image has an invalid size. Skipped."))?;

    let view_box = ViewBox {
        rect,
        aspect: node.attribute(AId::PreserveAspectRatio).unwrap_or_default(),
    };

    let href = node
        .attribute(AId::Href)
        .log_none(|| log::warn!("Image lacks the 'xlink:href' attribute. Skipped."))?;

    let data = get_href_data(href, state.opt)?;

    parent.append_kind(NodeKind::Image(Image {
        id: node.element_id().to_string(),
        transform: Default::default(),
        visibility,
        view_box,
        rendering_mode,
        data,
    }));

    Some(())
}

pub(crate) fn get_href_data(href: &str, opt: &OptionsRef) -> Option<Arc<PreloadedImageData>> {
    opt.image_data.get(href).map(Clone::clone)
}
