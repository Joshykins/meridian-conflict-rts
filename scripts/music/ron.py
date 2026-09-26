"""A small writer for the RON that mc-music reads.

Python values map to RON as: dict -> struct `(k: v, ...)`, list -> `[...]`,
tuple -> `(a, b)`, str -> "string", bool/int/float as numbers, `V(name, **fields)`
-> an enum variant (`Name`, `Name(k: v)`), `Newtype(name, value)` -> `Name(value)`.
`None` values in dicts are left out (the fields are optional or defaulted).
"""


class V:
    """An enum variant with named fields, or a unit variant when it has none."""

    def __init__(self, name, **fields):
        self.name = name
        self.fields = fields


class Newtype:
    """A newtype variant: `Synth((...))`."""

    def __init__(self, name, value):
        self.name = name
        self.value = value


def num(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, int):
        return str(x)
    s = f"{x:.4f}".rstrip("0").rstrip(".")
    if s in ("-0", ""):
        s = "0"
    if "." not in s:
        s += ".0"
    return s


def dump(v, indent=0, width=110):
    pad = "    " * indent
    inner = "    " * (indent + 1)
    if v is None:
        return "None"
    if isinstance(v, (bool, int, float)):
        return num(v)
    if isinstance(v, str):
        return '"' + v.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'
    if isinstance(v, tuple):
        return "(" + ", ".join(dump(x, indent + 1) for x in v) + ")"
    if isinstance(v, Newtype):
        return f"{v.name}({dump(v.value, indent)})"
    if isinstance(v, V):
        if not v.fields:
            return v.name
        body = dump_fields(v.fields, indent, width)
        return f"{v.name}{body}"
    if isinstance(v, dict):
        return dump_fields(v, indent, width)
    if isinstance(v, list):
        if not v:
            return "[]"
        parts = [dump(x, indent + 1) for x in v]
        one = "[" + ", ".join(parts) + "]"
        if len(one) + len(pad) <= width and "\n" not in one:
            return one
        # Pack short items several to a line (notes), long ones one per line.
        if all(len(p) < 30 and "\n" not in p for p in parts):
            lines, line = [], ""
            for p in parts:
                if line and len(line) + len(p) + 2 + len(inner) > width:
                    lines.append(line.rstrip())
                    line = ""
                line += p + ", "
            if line:
                lines.append(line.rstrip())
            return "[\n" + "\n".join(inner + l for l in lines) + "\n" + pad + "]"
        return "[\n" + "".join(inner + p + ",\n" for p in parts) + pad + "]"
    raise TypeError(f"cannot write {type(v)}")


def dump_fields(fields, indent, width):
    pad = "    " * indent
    inner = "    " * (indent + 1)
    items = [(k, x) for k, x in fields.items() if x is not None]
    parts = [f"{k}: {dump(x, indent + 1, width)}" for k, x in items]
    one = "(" + ", ".join(parts) + ")"
    if len(one) + len(pad) <= width and "\n" not in one:
        return one
    return "(\n" + "".join(inner + p + ",\n" for p in parts) + pad + ")"
