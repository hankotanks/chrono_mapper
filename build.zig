const std = @import("std");
const io = std.io;
const ArrayList = std.ArrayList;
const fs = std.fs;
const Level = std.log.Level;
const Build = std.Build;

const Pack = struct {
    step: *Build.Step,
    manifest: *Build.Step.Options,
    zjb: *Build.Dependency,
    zjb_art: *Build.Step.Run,

    fn init(b: *Build, comptime zjb_bridge_name: []const u8) @This() {
        const step = b.step("pack", "Accumulate web assets");
        // configure zjb
        const zjb = b.dependency("zjb", .{});
        const zjb_art = b.addRunArtifact(zjb.artifact("generate_js"));
        const zjb_out = zjb_art.addOutputFileArg(zjb_bridge_name);
        zjb_art.addArg("Zjb");
        // add JS bindings
        const bridge = b.addInstallFileWithDir(zjb_out, Build.InstallDir.bin, zjb_bridge_name);
        step.dependOn(&bridge.step);
        // return `Pack` object
        return .{
            .step = step,
            .manifest = b.addOptions(),
            .zjb = zjb,
            .zjb_art = zjb_art,
        };
    }

    fn include(self: *const @This(), b: *Build, path: Build.LazyPath) void {
        const dir = b.addInstallDirectory(.{
            .source_dir = path,
            .install_dir = Build.InstallDir.bin,
            .install_subdir = "",
        });
        self.step.dependOn(&dir.step);
    }

    // this function can fail, but always creates a manifest entry with 'name'
    fn includeWithManifest(
        self: *const @This(),
        b: *Build,
        comptime name: []const u8,
        path: Build.LazyPath,
    ) !void {
        var files = ArrayList([]const u8).init(b.allocator);
        defer files.deinit();
        // on failure, insert an empty manifest entry with this name
        errdefer self.manifest.addOption([]const []const u8, name, &[_][]const u8{});
        // open the asset directory
        var dir = try fs.cwd().openDir(name, .{ .iterate = true });
        defer dir.close();
        // create walker from IterableDir
        var walker = try dir.walk(b.allocator);
        defer walker.deinit();
        // recursively step through assets dir
        while (try walker.next()) |entry| {
            // only log files
            if (entry.kind != .file) {
                continue;
            }
            // replace backslashes (paths are web are '/' delimited)
            var entry_path = b.dupe(entry.path);
            for (0..entry_path.len) |i| {
                if (entry_path[i] == '\\') {
                    entry_path[i] = '/';
                }
            }
            // append path
            try files.append(entry_path);
        }
        // add to manifest
        self.manifest.addOption([]const []const u8, name, files.items);
        // add directory
        self.include(b, path);
    }

    fn addExecutable(
        self: *const @This(),
        b: *Build,
        comptime name: []const u8,
        path: Build.LazyPath,
        should_include_manifest: bool,
    ) void {
        // define target architecture
        const target = .{
            .cpu_arch = .wasm32,
            .os_tag = .freestanding,
        };
        // describe executable
        const exe = b.addExecutable(.{
            .name = name,
            .root_source_file = path,
            .target = b.resolveTargetQuery(target),
            .optimize = b.standardOptimizeOption(.{}),
        });
        exe.entry = .disabled;
        exe.rdynamic = true;
        exe.root_module.addImport("zjb", self.zjb.module("zjb"));
        // expose manifest to executable if requested
        if (should_include_manifest) {
            // self.manifest.addOptionPath("manifest", exe.getEmittedBin());
            exe.root_module.addOptions("manifest", self.manifest);
            // b.createModule(.{}).addOptions("manifest", self.manifest);
        }
        // expose JS bindings to executable
        self.zjb_art.addArtifactArg(exe);
        // add dependency to 'pack' step
        self.step.dependOn(&b.addInstallArtifact(exe, .{
            .dest_dir = .{
                .override = Build.InstallDir.bin,
            },
        }).step);
    }
};

pub fn build(b: *Build) void {
    const pack = Pack.init(b, "bridge.js");
    // if `zig build` is called, the 'pack' step is executed
    b.default_step = pack.step;
    // generates a WASM executable named 'core'
    pack.addExecutable(b, "core", b.path("src/main.zig"), true);
    // contains site root and scripts
    pack.include(b, b.path("web"));
    // add asset folder to output and build a manifest of its contents
    pack.includeWithManifest(b, "assets", b.path("assets")) catch {};
    // add 'host' step if it can be generated
    const host = buildHost(b, &[_]([]const u8){ "python", "python3" }) catch return;
    host.dependOn(b.default_step);
}

fn buildHost(
    b: *Build,
    comptime py_path_entries: []const []const u8,
) !*Build.Step {
    // prepare for failure if python isn't in PATH
    errdefer |err| {
        const writer = io.getStdErr().writer();
        // log the actual error
        writer.print("Configuration of 'host' step failed:\n\t{}\n", .{err}) catch {};
        writer.print("PATH must contain at least one of [", .{}) catch {};
        // log possible python PATH names
        var idx: usize = 0;
        for (py_path_entries) |item| {
            writer.print("{s}", .{item}) catch {};
            idx += 1;
            if (idx < py_path_entries.len) {
                writer.print(", ", .{}) catch {};
            }
        }
        writer.print("].\n", .{}) catch {};
    }
    // ensure python is in PATH
    const py_path = try b.findProgram(py_path_entries, &.{""});
    // configure `host` build step
    const host = b.step("host", "Statically serve project files");
    // allow the user to specify a port
    const port_default = 8080;
    const port_option_text = b.fmt("Specify the port (default {d})", .{port_default});
    const port = b.option(u32, "port", port_option_text) orelse port_default;
    // compose the command
    const host_command = b.addSystemCommand(&.{
        py_path,
        "-m",
        "http.server",
        b.fmt("{d}", .{port}),
        "--directory",
        b.getInstallPath(Build.InstallDir.bin, ""),
    });
    host.dependOn(&host_command.step);
    // return 'host' step to build
    return host;
}
