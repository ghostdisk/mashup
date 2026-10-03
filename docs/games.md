# Source game registry

The source-game registry in `mashup::assets::SOURCE_GAMES` defines stable short
namespace IDs and a concise list of observed/imported source formats. Asset IDs
use those namespaces (`hl:...`, `cstrike:...`, `gtasa:...`) so records from
different games cannot collide. A registry entry does not indicate that every
asset type or game behavior is implemented.

| ID | Source game | Import formats |
| --- | --- | --- |
| `hl` | Half-Life | GoldSrc MDL, GoldSrc BSP |
| `cstrike` | Counter-Strike 1.6 | GoldSrc MDL, GoldSrc BSP |
| `gtasa` | Grand Theft Auto: San Andreas | RenderWare DFF/TXD, GTA IMG/IPL/COL |
| `lineage2` | Lineage 2 | Importer pending |

GoldSrc model/map imports use the GLB plus `.import.json` sidecar. GTA world
content uses the versioned `mashup-world` package, including source provenance,
source coordinate basis, and meter scale. See [assets.md](assets.md),
[goldsrc-import.md](goldsrc-import.md), [gtasa-import.md](gtasa-import.md), and
[maps.md](maps.md). Installed source-game directories remain user-selected inputs
to explicit importers and are never probed by catalog discovery.
