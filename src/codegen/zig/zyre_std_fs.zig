const std = @import("std");

pub fn readTextFile(allocator: std.mem.Allocator, path: []const u8) ![]u8 {
    return std.fs.cwd().readFileAlloc(allocator, path, std.math.maxInt(usize));
}
