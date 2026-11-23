# Use time and system interruption (#3)

The scheduler advances use time without keyboard/mouse input. Lock and sleep are independent suspension flags: wake does not imply unlock. Interval changes begin a new interval; persisted configuration survives restart, while ordinary use time starts fresh. Interrupted round IDs and preview requests created before interruption cannot reopen a round.

macOS receives workspace sleep/wake and distributed screen lock/unlock notifications on the application thread. The `com.apple.screenIsLocked` and `com.apple.screenIsUnlocked` names are undocumented compatibility signals, isolated in `system_events.m`; support must be checked on each release target. Windows registers a hidden top-level window for WTS session and power notifications (not a message-only window). Events update the scheduler immediately, then queue the same presentation reconciliation that releases input and hides all windows.

Automated verification: scheduler public-operation tests cover due time with no input, restart, interval change, overlapping lock/sleep and wake while still locked, immediate interruption and rejection of old round completion/pending preview. Existing storage roundtrip verifies interval persistence. Native macOS test compilation includes the Objective-C bridge.

Pending packaged-app checks on both platforms: lock during a visible reminder, sleep during a visible reminder, wake without unlocking, unlock and confirm remaining use interval, rapid lock/unlock while preview windows initialize. Windows adapter and actual OS delivery have not been runtime validated by these tests. No physical lock/sleep operation is triggered by the automated suite.
