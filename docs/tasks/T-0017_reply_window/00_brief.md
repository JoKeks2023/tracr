# T-0017 Reply window

User request (2026-11-02): if the next prompt comes less than 5 minutes after Claude finished, the person counts as still active.

Before this change, real days showed 0 human minutes:
- **Device idle:** a gap counted only if the device was not idle for 5 minutes. Nobody types while Claude works, so long agent runs broke the bridge.
- **Session change:** switching to another session broke it too.
- **No sensor:** without the device sensor (app not running), every gap was a coverage gap.
- **Unknown record types:** since about 2026-10-22, every gap was also blocked as "degraded". Claude Code 2.1.267 writes new record types, tracr quarantined them, and each quarantined record marked its surroundings as degraded evidence.

Huly: TRACR-38.
