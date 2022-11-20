// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::hash;

use strict_num::PositiveF64;

use super::{Input, Kind, Primitive};
use crate::svgtree::{self, AId, EId};
use crate::{Color, ScreenRect, SvgColorExt, Transform};

/// A diffuse lighting filter primitive.
///
/// `feDiffuseLighting` element in the SVG.
#[derive(Clone, Debug)]
pub struct DiffuseLighting {
    /// Identifies input for the given filter primitive.
    ///
    /// `in` in the SVG.
    pub input: Input,

    /// A surface scale.
    ///
    /// `surfaceScale` in the SVG.
    pub surface_scale: f64,

    /// A diffuse constant.
    ///
    /// `diffuseConstant` in the SVG.
    pub diffuse_constant: f64,

    /// A lighting color.
    ///
    /// `lighting-color` in the SVG.
    pub lighting_color: Color,

    /// A light source.
    pub light_source: LightSource,
}

impl std::hash::Hash for DiffuseLighting {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.input.hash(state);
        self.surface_scale.to_bits().hash(state);
        self.diffuse_constant.to_bits().hash(state);
        self.lighting_color.hash(state);
        self.light_source.hash(state);
    }
}

pub(crate) fn convert_diffuse(fe: svgtree::Node, primitives: &[Primitive]) -> Option<Kind> {
    let light_source = convert_light_source(fe)?;
    Some(Kind::DiffuseLighting(DiffuseLighting {
        input: super::resolve_input(fe, AId::In, primitives),
        surface_scale: fe.attribute(AId::SurfaceScale).unwrap_or(1.0),
        diffuse_constant: fe.attribute(AId::DiffuseConstant).unwrap_or(1.0),
        lighting_color: convert_lighting_color(fe),
        light_source,
    }))
}

/// A specular lighting filter primitive.
///
/// `feSpecularLighting` element in the SVG.
#[derive(Clone, Debug)]
pub struct SpecularLighting {
    /// Identifies input for the given filter primitive.
    ///
    /// `in` in the SVG.
    pub input: Input,

    /// A surface scale.
    ///
    /// `surfaceScale` in the SVG.
    pub surface_scale: f64,

    /// A specular constant.
    ///
    /// `specularConstant` in the SVG.
    pub specular_constant: f64,

    /// A specular exponent.
    ///
    /// Should be in 1..128 range.
    ///
    /// `specularExponent` in the SVG.
    pub specular_exponent: f64,

    /// A lighting color.
    ///
    /// `lighting-color` in the SVG.
    pub lighting_color: Color,

    /// A light source.
    pub light_source: LightSource,
}

impl std::hash::Hash for SpecularLighting {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.input.hash(state);
        self.surface_scale.to_bits().hash(state);
        self.specular_constant.to_bits().hash(state);
        self.specular_exponent.to_bits().hash(state);
        self.lighting_color.hash(state);
        self.light_source.hash(state);
    }
}

pub(crate) fn convert_specular(fe: svgtree::Node, primitives: &[Primitive]) -> Option<Kind> {
    let light_source = convert_light_source(fe)?;

    let specular_exponent = fe.attribute(AId::SpecularExponent).unwrap_or(1.0);
    if !(1.0..=128.0).contains(&specular_exponent) {
        // When exponent is out of range, the whole filter primitive should be ignored.
        return None;
    }

    let specular_exponent = crate::utils::f64_bound(1.0, specular_exponent, 128.0);

    Some(Kind::SpecularLighting(SpecularLighting {
        input: super::resolve_input(fe, AId::In, primitives),
        surface_scale: fe.attribute(AId::SurfaceScale).unwrap_or(1.0),
        specular_constant: fe.attribute(AId::SpecularConstant).unwrap_or(1.0),
        specular_exponent,
        lighting_color: convert_lighting_color(fe),
        light_source,
    }))
}

#[inline(never)]
fn convert_lighting_color(node: svgtree::Node) -> Color {
    // Color's alpha doesn't affect lighting-color. Simply skip it.
    match node.attribute::<&svgtree::AttributeValue>(AId::LightingColor) {
        Some(svgtree::AttributeValue::CurrentColor) => {
            node.find_attribute(AId::Color)
                .unwrap_or_else(svgrtypes::Color::black)
                .split_alpha()
                .0
        }
        Some(svgtree::AttributeValue::Color(c)) => c.split_alpha().0,
        _ => Color::white(),
    }
}

/// A light source kind.
#[allow(missing_docs)]
#[derive(Clone, Hash, Copy, Debug)]
pub enum LightSource {
    DistantLight(DistantLight),
    PointLight(PointLight),
    SpotLight(SpotLight),
}

impl LightSource {
    /// Applies a transform to the light source.
    pub fn transform(mut self, region: ScreenRect, ts: &Transform) -> Self {
        use std::f64::consts::SQRT_2;

        match self {
            LightSource::DistantLight(..) => {}
            LightSource::PointLight(ref mut light) => {
                let (x, y) = ts.apply(light.x, light.y);
                light.x = x - region.x() as f64;
                light.y = y - region.y() as f64;
                light.z = light.z * (ts.a * ts.a + ts.d * ts.d).sqrt() / SQRT_2;
            }
            LightSource::SpotLight(ref mut light) => {
                let sz = (ts.a * ts.a + ts.d * ts.d).sqrt() / SQRT_2;

                let (x, y) = ts.apply(light.x, light.y);
                light.x = x - region.x() as f64;
                light.y = y - region.x() as f64;
                light.z *= sz;

                let (x, y) = ts.apply(light.points_at_x, light.points_at_y);
                light.points_at_x = x - region.x() as f64;
                light.points_at_y = y - region.x() as f64;
                light.points_at_z *= sz;
            }
        }

        self
    }
}

/// A distant light source.
///
/// `feDistantLight` element in the SVG.
#[derive(Clone, Copy, Debug)]
pub struct DistantLight {
    /// Direction angle for the light source on the XY plane (clockwise),
    /// in degrees from the x axis.
    ///
    /// `azimuth` in the SVG.
    pub azimuth: f64,

    /// Direction angle for the light source from the XY plane towards the z axis, in degrees.
    ///
    /// `elevation` in the SVG.
    pub elevation: f64,
}

impl hash::Hash for DistantLight {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.azimuth.to_bits().hash(state);
        self.elevation.to_bits().hash(state);
    }
}

/// A point light source.
///
/// `fePointLight` element in the SVG.
#[derive(Clone, Copy, Debug)]
pub struct PointLight {
    /// X location for the light source.
    ///
    /// `x` in the SVG.
    pub x: f64,

    /// Y location for the light source.
    ///
    /// `y` in the SVG.
    pub y: f64,

    /// Z location for the light source.
    ///
    /// `z` in the SVG.
    pub z: f64,
}

impl hash::Hash for PointLight {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.x.to_bits().hash(state);
        self.y.to_bits().hash(state);
        self.z.to_bits().hash(state);
    }
}

/// A spot light source.
///
/// `feSpotLight` element in the SVG.
#[derive(Clone, Copy, Debug)]
pub struct SpotLight {
    /// X location for the light source.
    ///
    /// `x` in the SVG.
    pub x: f64,

    /// Y location for the light source.
    ///
    /// `y` in the SVG.
    pub y: f64,

    /// Z location for the light source.
    ///
    /// `z` in the SVG.
    pub z: f64,

    /// X point at which the light source is pointing.
    ///
    /// `pointsAtX` in the SVG.
    pub points_at_x: f64,

    /// Y point at which the light source is pointing.
    ///
    /// `pointsAtY` in the SVG.
    pub points_at_y: f64,

    /// Z point at which the light source is pointing.
    ///
    /// `pointsAtZ` in the SVG.
    pub points_at_z: f64,

    /// Exponent value controlling the focus for the light source.
    ///
    /// `specularExponent` in the SVG.
    pub specular_exponent: PositiveF64,

    /// A limiting cone which restricts the region where the light is projected.
    ///
    /// `limitingConeAngle` in the SVG.
    pub limiting_cone_angle: Option<f64>,
}

impl hash::Hash for SpotLight {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.x.to_bits().hash(state);
        self.y.to_bits().hash(state);
        self.z.to_bits().hash(state);
        self.points_at_x.to_bits().hash(state);
        self.points_at_y.to_bits().hash(state);
        self.points_at_z.to_bits().hash(state);
        self.specular_exponent.hash(state);
        self.limiting_cone_angle.map(|v| v.to_bits().hash(state));
    }
}

#[inline(never)]
fn convert_light_source(parent: svgtree::Node) -> Option<LightSource> {
    let child = parent.children().find(|n| {
        matches!(
            n.tag_name(),
            Some(EId::FeDistantLight) | Some(EId::FePointLight) | Some(EId::FeSpotLight)
        )
    })?;

    match child.tag_name() {
        Some(EId::FeDistantLight) => Some(LightSource::DistantLight(DistantLight {
            azimuth: child.attribute(AId::Azimuth).unwrap_or(0.0),
            elevation: child.attribute(AId::Elevation).unwrap_or(0.0),
        })),
        Some(EId::FePointLight) => Some(LightSource::PointLight(PointLight {
            x: child.attribute(AId::X).unwrap_or(0.0),
            y: child.attribute(AId::Y).unwrap_or(0.0),
            z: child.attribute(AId::Z).unwrap_or(0.0),
        })),
        Some(EId::FeSpotLight) => {
            let specular_exponent = child.attribute(AId::SpecularExponent).unwrap_or(1.0);
            let specular_exponent = PositiveF64::new(specular_exponent)
                .unwrap_or_else(|| PositiveF64::new(1.0).unwrap());

            Some(LightSource::SpotLight(SpotLight {
                x: child.attribute(AId::X).unwrap_or(0.0),
                y: child.attribute(AId::Y).unwrap_or(0.0),
                z: child.attribute(AId::Z).unwrap_or(0.0),
                points_at_x: child.attribute(AId::PointsAtX).unwrap_or(0.0),
                points_at_y: child.attribute(AId::PointsAtY).unwrap_or(0.0),
                points_at_z: child.attribute(AId::PointsAtZ).unwrap_or(0.0),
                specular_exponent,
                limiting_cone_angle: child.attribute(AId::LimitingConeAngle),
            }))
        }
        _ => None,
    }
}