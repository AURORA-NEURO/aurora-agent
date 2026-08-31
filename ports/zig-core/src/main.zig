const std = @import("std");
const core = @import("lib.zig");

pub fn main() !void {
    const allocator = std.heap.page_allocator;
    const digest = try core.payloadDigestHex(allocator, "aurora-agent-fabric");
    defer allocator.free(digest);
    std.debug.print("aurora-zig-core digest={s}\n", .{digest});
}
