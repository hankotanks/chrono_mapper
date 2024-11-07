// TODO
// the `geom` module will only be used for triangulation
// in the future it will not depend on zjb, because it will
// not interact with JS handles
const zjb = @import("zjb");

// relevant types from ISO 19125
pub const Point = [2]f32;
pub const LinearRing = [][2]f32;
pub const Polygon = []LinearRing;
pub const MultiPolygon = []Polygon;

// types representing the data passed from JS
pub const Feature = struct { name: []u8, coordinates: MultiPolygon };
pub const FeatureLayer = struct { name: []u8, features: []Feature };

// draw polygon to canvas
pub fn renderPolygon(
    ctx: zjb.Handle,
    poly: Polygon,
    canvas_width: f32,
    canvas_height: f32,
) void {
    for (poly) |ring| {
        for (ring) |point| {
            const x = (point[0] + 180.0) * canvas_width / 360.0;
            const y = canvas_height - (point[1] + 90.0) * canvas_height / 180.0;
            ctx.call("fillRect", .{ x, y, 5, 5 }, void);
        }
    }
}
