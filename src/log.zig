const std = @import("std");
const Allocator = std.mem.Allocator;
const zjb = @import("zjb");

fn formatStringAsHandle(
    allocator: Allocator,
    comptime format: []const u8,
    args: anytype,
) zjb.Handle {
    var buf = std.ArrayList(u8).init(allocator);
    defer buf.deinit();
    buf.writer().print(format, args) catch {};
    return zjb.string(buf.items);
}

pub fn logInfo(
    allocator: Allocator,
    comptime format: []const u8,
    args: anytype,
) void {
    const handle = formatStringAsHandle(allocator, format, args);
    defer handle.release();
    const console = zjb.global("console");
    console.call("log", .{handle}, void);
}

pub fn logError(
    allocator: Allocator,
    comptime format: []const u8,
    args: anytype,
) void {
    const handle = formatStringAsHandle(allocator, format, args);
    defer handle.release();
    const console = zjb.global("console");
    console.call("error", .{handle}, void);
}
