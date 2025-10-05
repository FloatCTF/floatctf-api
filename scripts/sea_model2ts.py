import re
import sys
from pathlib import Path


def to_pascal_case(name: str) -> str:
    """snake_case → PascalCase"""
    return "".join(part.capitalize() for part in re.split(r"[_-]+", name) if part)


def parse_enum(rust_code: str) -> str:
    """
    把 ActiveEnum 转成 TS enum，保持原枚举名
    """
    output_blocks = []

    enum_pattern = re.compile(r"pub\s+enum\s+(\w+)\s*\{([\s\S]*?)\}", re.MULTILINE)

    for enum_name, body in enum_pattern.findall(rust_code):
        variant_pattern = re.compile(
            r'sea_orm\(string_value\s*=\s*"([^"]+)"\)\]\s*\n\s*(\w+)', re.MULTILINE
        )

        variants = variant_pattern.findall(body)

        if not variants:
            continue

        # 保持原有 Rust 枚举名
        lines = [f"  {var} = '{val}'," for val, var in variants]
        ts_block = f"export enum {enum_name} {{\n" + "\n".join(lines) + "\n}"
        output_blocks.append(ts_block)

    return "\n\n".join(output_blocks)


def rust_to_ts(rust_code: str) -> str:
    """
    把 SeaORM struct 转成 TS type
    """
    table_match = re.search(r'table_name\s*=\s*"([^"]+)"', rust_code)
    table_name = table_match.group(1) if table_match else None

    if not table_name:
        return ""

    type_name = to_pascal_case(table_name)
    field_pattern = re.compile(r"pub\s+(\w+)\s*:\s*([^,]+),")
    lines = []

    for name, rust_type in field_pattern.findall(rust_code):
        ts_type = "string"
        optional = False

        if re.search(r"Option\s*<\s*Uuid\s*>", rust_type):
            ts_type = "string"
            optional = True
        elif re.search(r"Uuid", rust_type):
            ts_type = "string"
        elif re.search(r"i32|i64|u32|u64|f32|f64", rust_type):
            ts_type = "number"
        elif re.search(r"String", rust_type):
            ts_type = "string"
        elif re.search(r"bool", rust_type):
            ts_type = "boolean"
        else:
            ts_type = "string"

        lines.append(f"  {name}{'?' if optional else ''}: {ts_type};")

    if not lines:
        return ""

    return f"export type {type_name} = {{\n" + "\n".join(lines) + "\n};\n"


def convert_directory(input_dir: str, output_dir: str):
    in_path = Path(input_dir)
    out_path = Path(output_dir)
    out_path.mkdir(parents=True, exist_ok=True)

    exports = []

    for rust_file in in_path.glob("*.rs"):
        rust_code = rust_file.read_text(encoding="utf-8")

        # 先尝试解析 ActiveEnum
        ts_code = parse_enum(rust_code)

        # 如果不是 enum，则尝试解析 struct
        if not ts_code.strip():
            ts_code = rust_to_ts(rust_code)

        if not ts_code.strip():
            print(f"跳过空文件: {rust_file.name}")
            continue

        ts_filename = rust_file.stem + ".ts"
        (out_path / ts_filename).write_text(ts_code, encoding="utf-8")
        print(f"已转换: {rust_file.name} → {ts_filename}")

        exports.append(rust_file.stem)

    # 生成 index.ts
    if exports:
        index_lines = [f'export * from "./{name}";' for name in exports]
        (out_path / "index.ts").write_text(
            "\n".join(index_lines) + "\n", encoding="utf-8"
        )
        print(f"已生成: {out_path / 'index.ts'}")
    else:
        print("⚠️ 没有生成任何 TypeScript 文件。")


def main():
    if len(sys.argv) != 3:
        print("用法: python rust2ts.py <输入目录> <输出目录>")
        sys.exit(1)

    convert_directory(sys.argv[1], sys.argv[2])


if __name__ == "__main__":
    main()
