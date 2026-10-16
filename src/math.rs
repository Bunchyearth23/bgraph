//! One portable implementation for math whose std precision varies by platform.
//!
//! The fixed libm version and force-soft-floats feature select the same algorithms
//! on every runtime. Primitive IEEE arithmetic, sqrt and exact rounding stay in std.

#[cfg(feature = "venn")]
pub(crate) use libm::acos;
#[cfg(feature = "pdf")]
pub(crate) use libm::tan;
pub(crate) use libm::{cos, hypot, log10, pow, sin};

pub(crate) fn square(value: f64) -> f64 {
    value * value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_kernels_keep_raw_bits_including_the_windows_circle_difference() {
        for (value, expected) in [
            (sin(1.0), 0x3feaed548f090cee),
            (cos(1.0), 0x3fe14a280fb5068c),
            (sin(std::f64::consts::TAU * 11.0 / 32.0), 0x3fea9b66290ea1a4),
            (sin(1e100), 0xbfd85c5e5b929359),
            (hypot(0.1, 0.2), 0x3fcc9f25c5bfedd9),
            (pow(1.25, 2.4), 0x3ffb55846c04d3ec),
            (log10(1.25), 0x3fb8cf183886480d),
            (sin(-0.0), 0x8000000000000000),
        ] {
            assert_eq!(value.to_bits(), expected);
        }
        #[cfg(feature = "venn")]
        assert_eq!(acos(0.125).to_bits(), 0x3ff720a392c1d954);
        #[cfg(feature = "pdf")]
        assert_eq!(tan(0.125).to_bits(), 0x3fc01577af1511a5);
    }

    #[test]
    fn geometry_calls_the_portable_kernels_instead_of_platform_math() {
        for source in [
            include_str!("charts.rs"),
            include_str!("bpmn.rs"),
            include_str!("render_svg.rs"),
            include_str!("render_pdf.rs"),
            include_str!("layout.rs"),
        ] {
            for method in [
                "sin", "cos", "tan", "acos", "hypot", "powf", "powi", "log10",
            ] {
                assert!(
                    !source.contains(&format!(".{method}(")),
                    "platform-dependent {method} reintroduced in geometry"
                );
            }
        }
        let manifest = include_str!("../Cargo.toml");
        let dependency = manifest
            .lines()
            .find(|line| line.starts_with("libm = "))
            .unwrap();
        assert!(dependency.contains("version = \"=0.2.16\""));
        assert!(dependency.contains("default-features = false"));
        assert!(dependency.contains("force-soft-floats"));
    }
}
