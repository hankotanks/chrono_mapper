const std = @import("std");
const json = std.json;
const alloc = std.heap.wasm_allocator;
const zjb = @import("zjb");
const log = @import("log.zig");
const geom = @import("geom.zig");

export fn allocArrayBuffer(n: i32, bytes_per_element: i32) callconv(.C) i32 {
    const size: usize = @intCast(n * bytes_per_element);
    const ptr = alloc.alloc(u8, size) catch {
        return -1;
    };
    return @intCast(@intFromPtr(&ptr[0]));
}
comptime {
    zjb.exportFn("allocArrayBuffer", allocArrayBuffer);
}

fn sliceArrayBuffer(comptime T: type, p: i32, n: i32) []T {
    const length = @as(usize, @intCast(n));
    const temp: usize = @bitCast(p);
    return @as([*]T, @ptrFromInt(temp))[0..length];
}

export fn getLayers() callconv(.C) zjb.Handle {
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
    zjb.exportFn("getLayers", getLayers);
}

inline fn parseLayer(offset: i32, count: i32) !geom.FeatureLayer {
    // get slice containing JSON
    const layer = sliceArrayBuffer(u8, offset, count);
    defer alloc.free(layer);
    // attempt to parse
    const parsed = try json.parseFromSlice(geom.FeatureLayer, alloc, layer, .{});
    // on success, return the parsed FeatureLayer
    return parsed.value;
}

fn renderLayerInner(
    ctx: zjb.Handle,
    offset: i32,
    count: i32,
    canvas_width: f32,
    canvas_height: f32,
) !void {
    defer ctx.release();
    // try to parse the layer
    const layer = try parseLayer(offset, count);
    // for each polygon in the layer, render
    for (layer.features) |feature| {
        for (feature.coordinates) |poly| {
            geom.renderPolygon(ctx, poly, canvas_width, canvas_height);
        }
    }
}

export fn renderLayer(
    ctx: zjb.Handle,
    offset: i32,
    count: i32,
    canvas_width: f32,
    canvas_height: f32,
) i32 {
    // execute failable render
    renderLayerInner(
        ctx,
        offset,
        count,
        canvas_width,
        canvas_height,
    ) catch |err| {
        // if parsing failed
        log.logError(alloc, "{}", .{err});
        return 1;
    };
    // return 0 on success
    return 0;
}
comptime {
    zjb.exportFn("renderLayer", renderLayer);
}
