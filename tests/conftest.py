import pytest

import monadb


@pytest.fixture(params=["memory", "file"])
def db(request, tmp_path):
    if request.param == "memory":
        d = monadb.open()
    else:
        d = monadb.open(str(tmp_path / "t.db"))
    yield d
    d.close()


def rank(key):
    """Reference ordering: mirrors the tag-byte key codec (int < str < bytes)."""
    parts = key if isinstance(key, tuple) else (key,)
    out = []
    for p in parts:
        if isinstance(p, bool):
            raise TypeError("bool is not a key type")
        if isinstance(p, int):
            out.append((1, p))
        elif isinstance(p, str):
            out.append((2, tuple(p.encode("utf-8"))))
        elif isinstance(p, bytes):
            out.append((3, tuple(p)))
        else:
            raise TypeError(f"bad key component: {type(p)}")
    return out
