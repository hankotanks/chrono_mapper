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
