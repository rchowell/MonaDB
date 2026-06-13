from mona import Mona

mo = Mona(api_key="...")


ns = mo.namespace("example")

ns.create_table("foo", x=int, y=str)

ns.insert("foo", [
    {"x": 1, "y": "a"},
    {"x": 2, "y": "b"},
    {"x": 3, "y": "c"},
])

ns.get("foo", x=1)
ns.delete("foo", x=1)
