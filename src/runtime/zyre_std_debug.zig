const std = @import("std");

pub fn print(val: anytype) void {
    if (comptime @typeInfo(@TypeOf(val)) == .pointer) {
        std.debug.print("{s}\n", .{val});
    } else {
        std.debug.print("{any}\n", .{val});
    }
}
