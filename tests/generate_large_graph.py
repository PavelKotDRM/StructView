"""Generate a deterministic, layered relationship graph for routing benchmarks."""

import json
import sys
from pathlib import Path


LAYER_COUNT = 12
NODES_PER_LAYER = 20
TARGET_OFFSETS = (0, 3, 6, 9, 12, 15)
RELATION_TYPES = (
    "calls",
    "reads",
    "writes",
    "publishes",
    "depends on",
    "notifies",
)


def main() -> None:
    output = (
        Path(sys.argv[1])
        if len(sys.argv) > 1
        else Path(__file__).parent / "fixtures" / "large-routing-graph.json"
    )
    nodes = {
        f"service-{layer:02}-{slot:02}": f"Service {layer:02}-{slot:02}"
        for layer in range(LAYER_COUNT)
        for slot in range(NODES_PER_LAYER)
    }
    edges = []
    for layer in range(LAYER_COUNT - 1):
        for slot in range(NODES_PER_LAYER):
            source = f"service-{layer:02}-{slot:02}"
            for relation_index, offset in enumerate(TARGET_OFFSETS):
                target_slot = (slot + offset) % NODES_PER_LAYER
                target = f"service-{layer + 1:02}-{target_slot:02}"
                edges.append(
                    [
                        source,
                        target,
                        RELATION_TYPES[relation_index],
                    ]
                )

    graph = {
        "graph": {
            "name": "Synthetic layered routing benchmark",
            "directed": True,
            "entity_count": len(nodes),
            "relation_count": len(edges),
        },
        "entities": nodes,
        "relations": {"edges": edges},
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(graph, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"Wrote {len(nodes)} entities and {len(edges)} relationships to {output}")


if __name__ == "__main__":
    main()
