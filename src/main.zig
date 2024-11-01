const std = @import("std");
const alloc = std.heap.wasm_allocator;
const zjb = @import("zjb");
const log = @import("log.zig");

export fn getFeatures() callconv(.C) zjb.Handle {
    const manifest = @import("manifest");
    // instantiate a JS array object
    const list = zjb.global("Array").new(.{});
    for (manifest.assets) |asset| {
        // this method returns the length of the array
        // which we can safely discard
        _ = list.call("push", .{zjb.string(asset)}, i32);
    }
    return list;
}
comptime {
    zjb.exportFn("getFeatures", getFeatures);
}

export fn allocArray(n: i32) callconv(.C) i32 {
    const ptr = (alloc.alloc(u32, @intCast(n))) catch {
        return -1;
    };
    return @intCast(@intFromPtr(&ptr[0]));
}
comptime {
    zjb.exportFn("allocArray", allocArray);
}

fn castSlice(comptime T: type, p: i32, n: i32) []T {
    const length = @as(usize, @intCast(n));
    const temp: usize = @bitCast(p);
    return @as([*]T, @ptrFromInt(temp))[0..length];
}

export fn plotFeature(
    ctx: zjb.Handle,
    point_offset: i32,
    point_count: i32,
    ring_offset: i32,
    ring_count: i32,
    canvas_width: f32,
    canvas_height: f32,
) void {
    defer ctx.release();
    const points = castSlice(f32, point_offset, point_count);
    defer alloc.free(points);
    const rings = castSlice(u32, ring_offset, ring_count);
    defer alloc.free(rings);
    for (0..(points.len / 2)) |i| {
        const x = (points[i * 2] + 180.0) * canvas_width / 360.0;
        const y = canvas_height - (points[i * 2 + 1] + 90.0) * canvas_height / 180.0;
        ctx.call("fillRect", .{ x, y, 5, 5 }, void);
    }
}
comptime {
    zjb.exportFn("plotFeature", plotFeature);
}
