const std = @import("std");
const alloc = std.heap.wasm_allocator;
const zjb = @import("zjb");
const log = @import("log.zig");

const manifest = @import("manifest");

export fn getFeatures() callconv(.C) zjb.Handle {
    const list = zjb.global("Array").new(.{});
    for (manifest.assets) |asset| {
        _ = list.call("push", .{zjb.string(asset)}, i32);
    }
    return list;
}
comptime {
    zjb.exportFn("getFeatures", getFeatures);
}

export fn plotPoint(ctx: zjb.Handle, x: f32, y: f32, w: f32, h: f32) callconv(.C) void {
    defer ctx.release();
    const x0 = (x + 180.0) * w / 360.0;
    const y0 = h - (y + 90.0) * h / 180.0;
    ctx.call("fillRect", .{ x0, y0, 5, 5 }, void);
}
comptime {
    zjb.exportFn("plotPoint", plotPoint);
}
