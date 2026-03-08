use super::{err, ok};

#[test]
fn test_fs_read_file() {
    ok(r#"
        const std = import("std");
        fn main(): void {
            const data = std.fs.readTextFile("test.txt") catch err {
                return;
            };
            std.debug.print(data);
        }
    "#);
}

#[test]
fn test_fs_read_file_propagate() {
    ok(r#"
        const std = import("std");
        fn readContent(): !string {
            const data = std.fs.readTextFile("test.txt")?;
            return data;
        }
        fn main(): void {}
    "#);
}

#[test]
fn test_fs_read_file_arg_count() {
    err(
        r#"
        const std = import("std");
        fn main(): void {
            const data = std.fs.readTextFile("a", "b") catch err {
                return;
            };
        }
    "#,
        "std.fs.readTextFile expects 1 argument",
    );
}

#[test]
fn test_parse_alloc_propagate() {
    ok(r#"
        const std = import("std");
        fn readContent(): !string {
            const data = std.fs.readTextFile("test.txt")?;
            return data;
        }
        fn main(): void {
            const data = readContent() catch err { return; };
            std.debug.print(data);
        }
    "#);
}
