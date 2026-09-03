//! Zig hot-path parity layer for Aurora's dependency-free agent-fabric primitives.
//!
//! This is intentionally not a scheduler or a distributed runtime. It ports the small operations
//! that are useful at an edge boundary: typed nonzero identifiers, capability normalization,
//! SHA-256 payload binding, deterministic JSON field ordering, hex encoding, and bounded queues.
//! The Rust integration remains the semantic reference until the parity tests are run on the same
//! vector set with a pinned Zig toolchain.

const std = @import("std");

pub const IdError = error{InvalidId};

pub const AgentId = struct {
    raw_value: u64,
    pub fn init(raw: u64) IdError!AgentId {
        if (raw == 0) return error.InvalidId;
        return .{ .raw_value = raw };
    }
    pub fn raw(self: AgentId) u64 { return self.raw_value; }
};

pub const TaskId = struct {
    raw_value: u64,
    pub fn init(raw: u64) IdError!TaskId {
        if (raw == 0) return error.InvalidId;
        return .{ .raw_value = raw };
    }
    pub fn raw(self: TaskId) u64 { return self.raw_value; }
};

pub const ShardId = struct {
    raw_value: u64,
    pub fn init(raw: u64) IdError!ShardId {
        if (raw == 0) return error.InvalidId;
        return .{ .raw_value = raw };
    }
    pub fn raw(self: ShardId) u64 { return self.raw_value; }
};

pub const CapabilityError = error{ Empty, TooLong, NonAscii, IllegalCharacter };
const max_capability_len = 128;

pub fn normalizeCapability(allocator: std.mem.Allocator, input: []const u8) CapabilityError![]u8 {
    if (input.len == 0) return error.Empty;
    if (input.len > max_capability_len) return error.TooLong;
    const output = try allocator.alloc(u8, input.len);
    errdefer allocator.free(output);
    for (input, 0..) |byte, index| {
        if (byte >= 0x80) return error.NonAscii;
        output[index] = switch (byte) {
            'A'...'Z' => byte + ('a' - 'A'),
            'a'...'z', '0'...'9', '.', '-', '_' => byte,
            else => return error.IllegalCharacter,
        };
    }
    return output;
}

pub const Digest = [32]u8;

pub fn sha256(payload: []const u8) Digest {
    var digest: Digest = undefined;
    std.crypto.hash.sha2.Sha256.hash(payload, &digest, .{});
    return digest;
}

pub fn hexEncode(allocator: std.mem.Allocator, bytes: []const u8) ![]u8 {
    const out = try allocator.alloc(u8, bytes.len * 2);
    const digits = "0123456789abcdef";
    for (bytes, 0..) |byte, index| {
        out[index * 2] = digits[byte >> 4];
        out[index * 2 + 1] = digits[byte & 0x0f];
    }
    return out;
}

pub fn payloadDigestHex(allocator: std.mem.Allocator, payload: []const u8) ![]u8 {
    return hexEncode(allocator, &sha256(payload));
}

pub const CanonicalField = struct {
    key: []const u8,
    value_json: []const u8,
};

fn fieldLessThan(_: void, left: CanonicalField, right: CanonicalField) bool {
    return std.mem.lessThan(u8, left.key, right.key);
}

/// Writes a compact object with lexicographically sorted keys. Values are already-encoded JSON;
/// callers must provide valid JSON and must not use this as a general parser.
pub fn canonicalObject(allocator: std.mem.Allocator, fields: []const CanonicalField) ![]u8 {
    const sorted = try allocator.dupe(CanonicalField, fields);
    defer allocator.free(sorted);
    std.mem.sort(CanonicalField, sorted, {}, fieldLessThan);
    var output = std.ArrayList(u8).init(allocator);
    errdefer output.deinit();
    try output.append('{');
    for (sorted, 0..) |field, index| {
        if (index != 0) try output.append(',');
        try output.append('"');
        for (field.key) |byte| {
            switch (byte) {
                '"' => try output.appendSlice("\\\""),
                '\\' => try output.appendSlice("\\\\"),
                '\n' => try output.appendSlice("\\n"),
                '\r' => try output.appendSlice("\\r"),
                '\t' => try output.appendSlice("\\t"),
                else => try output.append(byte),
            }
        }
        try output.appendSlice("\":");
        try output.appendSlice(field.value_json);
    }
    try output.append('}');
    return output.toOwnedSlice();
}

pub fn BoundedQueue(comptime T: type) type {
    return struct {
        const Self = @This();
        allocator: std.mem.Allocator,
        storage: []T,
        head: usize = 0,
        len: usize = 0,

        pub fn init(allocator: std.mem.Allocator, capacity: usize) !Self {
            if (capacity == 0) return error.ZeroCapacity;
            return .{ .allocator = allocator, .storage = try allocator.alloc(T, capacity) };
        }

        pub fn deinit(self: *Self) void { self.allocator.free(self.storage); }
        pub fn capacity(self: *const Self) usize { return self.storage.len; }
        pub fn count(self: *const Self) usize { return self.len; }

        pub fn push(self: *Self, item: T) error{Backpressure}!void {
            if (self.len == self.storage.len) return error.Backpressure;
            const slot = (self.head + self.len) % self.storage.len;
            self.storage[slot] = item;
            self.len += 1;
        }

        pub fn pop(self: *Self) ?T {
            if (self.len == 0) return null;
            const item = self.storage[self.head];
            self.head = (self.head + 1) % self.storage.len;
            self.len -= 1;
            return item;
        }
    };
}

test "zero identifiers are rejected and valid identifiers retain their raw value" {
    try std.testing.expectError(error.InvalidId, AgentId.init(0));
    const task = try TaskId.init(7);
    try std.testing.expectEqual(@as(u64, 7), task.raw());
}

test "capability normalization matches the Rust alphabet and case rule" {
    const allocator = std.testing.allocator;
    const cap = try normalizeCapability(allocator, "Genomics.Align-2");
    defer allocator.free(cap);
    try std.testing.expectEqualStrings("genomics.align-2", cap);
    try std.testing.expectError(error.IllegalCharacter, normalizeCapability(allocator, "a b"));
}

test "sha256 and hex encoding have the published hello vector" {
    const allocator = std.testing.allocator;
    const hex = try payloadDigestHex(allocator, "hello");
    defer allocator.free(hex);
    try std.testing.expectEqualStrings("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824", hex);
}

test "canonical object sorts keys and bounded queue reports backpressure" {
    const allocator = std.testing.allocator;
    const object = try canonicalObject(allocator, &[_]CanonicalField{
        .{ .key = "b", .value_json = "2" },
        .{ .key = "a", .value_json = "1" },
    });
    defer allocator.free(object);
    try std.testing.expectEqualStrings("{\"a\":1,\"b\":2}", object);

    var queue = try BoundedQueue(u8).init(allocator, 1);
    defer queue.deinit();
    try queue.push(9);
    try std.testing.expectError(error.Backpressure, queue.push(10));
    try std.testing.expectEqual(@as(?u8, 9), queue.pop());
}
