// =============================================================================
// color_names.rs - CSS named colors (W3C CSS Color Module Level 4)
// 148 named colors with nearest-color lookup
// =============================================================================

/// A CSS named color with its RGB components.
struct NamedColor {
    name: &'static str,
    r: u8,
    g: u8,
    b: u8,
}

/// All 148 CSS named colors (excluding grey/gray duplicates).
static CSS_COLORS: &[NamedColor] = &[
    NamedColor { name: "aliceblue", r: 240, g: 248, b: 255 },
    NamedColor { name: "antiquewhite", r: 250, g: 235, b: 215 },
    NamedColor { name: "aqua", r: 0, g: 255, b: 255 },
    NamedColor { name: "aquamarine", r: 127, g: 255, b: 212 },
    NamedColor { name: "azure", r: 240, g: 255, b: 255 },
    NamedColor { name: "beige", r: 245, g: 245, b: 220 },
    NamedColor { name: "bisque", r: 255, g: 228, b: 196 },
    NamedColor { name: "black", r: 0, g: 0, b: 0 },
    NamedColor { name: "blanchedalmond", r: 255, g: 235, b: 205 },
    NamedColor { name: "blue", r: 0, g: 0, b: 255 },
    NamedColor { name: "blueviolet", r: 138, g: 43, b: 226 },
    NamedColor { name: "brown", r: 165, g: 42, b: 42 },
    NamedColor { name: "burlywood", r: 222, g: 184, b: 135 },
    NamedColor { name: "cadetblue", r: 95, g: 158, b: 160 },
    NamedColor { name: "chartreuse", r: 127, g: 255, b: 0 },
    NamedColor { name: "chocolate", r: 210, g: 105, b: 30 },
    NamedColor { name: "coral", r: 255, g: 127, b: 80 },
    NamedColor { name: "cornflowerblue", r: 100, g: 149, b: 237 },
    NamedColor { name: "cornsilk", r: 255, g: 248, b: 220 },
    NamedColor { name: "crimson", r: 220, g: 20, b: 60 },
    NamedColor { name: "cyan", r: 0, g: 255, b: 255 },
    NamedColor { name: "darkblue", r: 0, g: 0, b: 139 },
    NamedColor { name: "darkcyan", r: 0, g: 139, b: 139 },
    NamedColor { name: "darkgoldenrod", r: 184, g: 134, b: 11 },
    NamedColor { name: "darkgray", r: 169, g: 169, b: 169 },
    NamedColor { name: "darkgreen", r: 0, g: 100, b: 0 },
    NamedColor { name: "darkkhaki", r: 189, g: 183, b: 107 },
    NamedColor { name: "darkmagenta", r: 139, g: 0, b: 139 },
    NamedColor { name: "darkolivegreen", r: 85, g: 107, b: 47 },
    NamedColor { name: "darkorange", r: 255, g: 140, b: 0 },
    NamedColor { name: "darkorchid", r: 153, g: 50, b: 204 },
    NamedColor { name: "darkred", r: 139, g: 0, b: 0 },
    NamedColor { name: "darksalmon", r: 233, g: 150, b: 122 },
    NamedColor { name: "darkseagreen", r: 143, g: 188, b: 143 },
    NamedColor { name: "darkslateblue", r: 72, g: 61, b: 139 },
    NamedColor { name: "darkslategray", r: 47, g: 79, b: 79 },
    NamedColor { name: "darkturquoise", r: 0, g: 206, b: 209 },
    NamedColor { name: "darkviolet", r: 148, g: 0, b: 211 },
    NamedColor { name: "deeppink", r: 255, g: 20, b: 147 },
    NamedColor { name: "deepskyblue", r: 0, g: 191, b: 255 },
    NamedColor { name: "dimgray", r: 105, g: 105, b: 105 },
    NamedColor { name: "dodgerblue", r: 30, g: 144, b: 255 },
    NamedColor { name: "firebrick", r: 178, g: 34, b: 34 },
    NamedColor { name: "floralwhite", r: 255, g: 250, b: 240 },
    NamedColor { name: "forestgreen", r: 34, g: 139, b: 34 },
    NamedColor { name: "fuchsia", r: 255, g: 0, b: 255 },
    NamedColor { name: "gainsboro", r: 220, g: 220, b: 220 },
    NamedColor { name: "ghostwhite", r: 248, g: 248, b: 255 },
    NamedColor { name: "gold", r: 255, g: 215, b: 0 },
    NamedColor { name: "goldenrod", r: 218, g: 165, b: 32 },
    NamedColor { name: "gray", r: 128, g: 128, b: 128 },
    NamedColor { name: "green", r: 0, g: 128, b: 0 },
    NamedColor { name: "greenyellow", r: 173, g: 255, b: 47 },
    NamedColor { name: "honeydew", r: 240, g: 255, b: 240 },
    NamedColor { name: "hotpink", r: 255, g: 105, b: 180 },
    NamedColor { name: "indianred", r: 205, g: 92, b: 92 },
    NamedColor { name: "indigo", r: 75, g: 0, b: 130 },
    NamedColor { name: "ivory", r: 255, g: 255, b: 240 },
    NamedColor { name: "khaki", r: 240, g: 230, b: 140 },
    NamedColor { name: "lavender", r: 230, g: 230, b: 250 },
    NamedColor { name: "lavenderblush", r: 255, g: 240, b: 245 },
    NamedColor { name: "lawngreen", r: 124, g: 252, b: 0 },
    NamedColor { name: "lemonchiffon", r: 255, g: 250, b: 205 },
    NamedColor { name: "lightblue", r: 173, g: 216, b: 230 },
    NamedColor { name: "lightcoral", r: 240, g: 128, b: 128 },
    NamedColor { name: "lightcyan", r: 224, g: 255, b: 255 },
    NamedColor { name: "lightgoldenrodyellow", r: 250, g: 250, b: 210 },
    NamedColor { name: "lightgray", r: 211, g: 211, b: 211 },
    NamedColor { name: "lightgreen", r: 144, g: 238, b: 144 },
    NamedColor { name: "lightpink", r: 255, g: 182, b: 193 },
    NamedColor { name: "lightsalmon", r: 255, g: 160, b: 122 },
    NamedColor { name: "lightseagreen", r: 32, g: 178, b: 170 },
    NamedColor { name: "lightskyblue", r: 135, g: 206, b: 250 },
    NamedColor { name: "lightslategray", r: 119, g: 136, b: 153 },
    NamedColor { name: "lightsteelblue", r: 176, g: 196, b: 222 },
    NamedColor { name: "lightyellow", r: 255, g: 255, b: 224 },
    NamedColor { name: "lime", r: 0, g: 255, b: 0 },
    NamedColor { name: "limegreen", r: 50, g: 205, b: 50 },
    NamedColor { name: "linen", r: 250, g: 240, b: 230 },
    NamedColor { name: "magenta", r: 255, g: 0, b: 255 },
    NamedColor { name: "maroon", r: 128, g: 0, b: 0 },
    NamedColor { name: "mediumaquamarine", r: 102, g: 205, b: 170 },
    NamedColor { name: "mediumblue", r: 0, g: 0, b: 205 },
    NamedColor { name: "mediumorchid", r: 186, g: 85, b: 211 },
    NamedColor { name: "mediumpurple", r: 147, g: 112, b: 219 },
    NamedColor { name: "mediumseagreen", r: 60, g: 179, b: 113 },
    NamedColor { name: "mediumslateblue", r: 123, g: 104, b: 238 },
    NamedColor { name: "mediumspringgreen", r: 0, g: 250, b: 154 },
    NamedColor { name: "mediumturquoise", r: 72, g: 209, b: 204 },
    NamedColor { name: "mediumvioletred", r: 199, g: 21, b: 133 },
    NamedColor { name: "midnightblue", r: 25, g: 25, b: 112 },
    NamedColor { name: "mintcream", r: 245, g: 255, b: 250 },
    NamedColor { name: "mistyrose", r: 255, g: 228, b: 225 },
    NamedColor { name: "moccasin", r: 255, g: 228, b: 181 },
    NamedColor { name: "navajowhite", r: 255, g: 222, b: 173 },
    NamedColor { name: "navy", r: 0, g: 0, b: 128 },
    NamedColor { name: "oldlace", r: 253, g: 245, b: 230 },
    NamedColor { name: "olive", r: 128, g: 128, b: 0 },
    NamedColor { name: "olivedrab", r: 107, g: 142, b: 35 },
    NamedColor { name: "orange", r: 255, g: 165, b: 0 },
    NamedColor { name: "orangered", r: 255, g: 69, b: 0 },
    NamedColor { name: "orchid", r: 218, g: 112, b: 214 },
    NamedColor { name: "palegoldenrod", r: 238, g: 232, b: 170 },
    NamedColor { name: "palegreen", r: 152, g: 251, b: 152 },
    NamedColor { name: "paleturquoise", r: 175, g: 238, b: 238 },
    NamedColor { name: "palevioletred", r: 219, g: 112, b: 147 },
    NamedColor { name: "papayawhip", r: 255, g: 239, b: 213 },
    NamedColor { name: "peachpuff", r: 255, g: 218, b: 185 },
    NamedColor { name: "peru", r: 205, g: 133, b: 63 },
    NamedColor { name: "pink", r: 255, g: 192, b: 203 },
    NamedColor { name: "plum", r: 221, g: 160, b: 221 },
    NamedColor { name: "powderblue", r: 176, g: 224, b: 230 },
    NamedColor { name: "purple", r: 128, g: 0, b: 128 },
    NamedColor { name: "rebeccapurple", r: 102, g: 51, b: 153 },
    NamedColor { name: "red", r: 255, g: 0, b: 0 },
    NamedColor { name: "rosybrown", r: 188, g: 143, b: 143 },
    NamedColor { name: "royalblue", r: 65, g: 105, b: 225 },
    NamedColor { name: "saddlebrown", r: 139, g: 69, b: 19 },
    NamedColor { name: "salmon", r: 250, g: 128, b: 114 },
    NamedColor { name: "sandybrown", r: 244, g: 164, b: 96 },
    NamedColor { name: "seagreen", r: 46, g: 139, b: 87 },
    NamedColor { name: "seashell", r: 255, g: 245, b: 238 },
    NamedColor { name: "sienna", r: 160, g: 82, b: 45 },
    NamedColor { name: "silver", r: 192, g: 192, b: 192 },
    NamedColor { name: "skyblue", r: 135, g: 206, b: 235 },
    NamedColor { name: "slateblue", r: 106, g: 90, b: 205 },
    NamedColor { name: "slategray", r: 112, g: 128, b: 144 },
    NamedColor { name: "snow", r: 255, g: 250, b: 250 },
    NamedColor { name: "springgreen", r: 0, g: 255, b: 127 },
    NamedColor { name: "steelblue", r: 70, g: 130, b: 180 },
    NamedColor { name: "tan", r: 210, g: 180, b: 140 },
    NamedColor { name: "teal", r: 0, g: 128, b: 128 },
    NamedColor { name: "thistle", r: 216, g: 191, b: 216 },
    NamedColor { name: "tomato", r: 255, g: 99, b: 71 },
    NamedColor { name: "turquoise", r: 64, g: 224, b: 208 },
    NamedColor { name: "violet", r: 238, g: 130, b: 238 },
    NamedColor { name: "wheat", r: 245, g: 222, b: 179 },
    NamedColor { name: "white", r: 255, g: 255, b: 255 },
    NamedColor { name: "whitesmoke", r: 245, g: 245, b: 245 },
    NamedColor { name: "yellow", r: 255, g: 255, b: 0 },
    NamedColor { name: "yellowgreen", r: 154, g: 205, b: 50 },
];

/// Returns the nearest CSS color name for a given RGB value.
///
/// Uses Euclidean distance in RGB space. Returns the exact name
/// if there is a perfect match, otherwise the closest one.
pub fn nearest_color_name(r: u8, g: u8, b: u8) -> &'static str {
    let (r, g, b) = (r as i32, g as i32, b as i32);
    let mut best_name = "black";
    let mut best_dist = i32::MAX;

    for c in CSS_COLORS {
        let dr = r - c.r as i32;
        let dg = g - c.g as i32;
        let db = b - c.b as i32;
        let dist = dr * dr + dg * dg + db * db;
        if dist == 0 {
            return c.name;
        }
        if dist < best_dist {
            best_dist = dist;
            best_name = c.name;
        }
    }

    best_name
}

/// Returns the exact CSS color name if the RGB value matches one, or None.
pub fn exact_color_name(r: u8, g: u8, b: u8) -> Option<&'static str> {
    CSS_COLORS.iter().find(|c| c.r == r && c.g == g && c.b == b).map(|c| c.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert_eq!(nearest_color_name(255, 0, 0), "red");
        assert_eq!(nearest_color_name(0, 0, 0), "black");
        assert_eq!(nearest_color_name(255, 255, 255), "white");
        assert_eq!(nearest_color_name(255, 165, 0), "orange");
    }

    #[test]
    fn test_nearest_match() {
        // Close to red
        assert_eq!(nearest_color_name(254, 1, 1), "red");
        // Close to navy
        assert_eq!(nearest_color_name(0, 0, 130), "navy");
    }

    #[test]
    fn test_exact_color_name() {
        assert_eq!(exact_color_name(255, 0, 0), Some("red"));
        assert_eq!(exact_color_name(254, 0, 0), None);
    }
}
