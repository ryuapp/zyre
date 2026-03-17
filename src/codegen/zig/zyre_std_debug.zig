const std = @import("std");

pub fn print(args: anytype) void {
    const ArgsType = @TypeOf(args);
    const info = @typeInfo(ArgsType);
    if (info != .@"struct") @compileError("print: expected tuple");
    inline for (info.@"struct".fields, 0..) |field, i| {
        if (i > 0) std.debug.print(" ", .{});
        const val = @field(args, field.name);
        if (comptime @typeInfo(@TypeOf(val)) == .pointer) {
            std.debug.print("{s}", .{val});
        } else {
            std.debug.print("{any}", .{val});
        }
    }
    std.debug.print("\n", .{});
}
