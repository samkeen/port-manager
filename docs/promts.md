```
I run a lot of apps locally.  I loose track of what is running on what port.
i want to create a cli tool to show what is running on each port, and give me the option to kill the process  
use https://github.com/ratatui/ratatui
```
[~3 min] App ran with no errors

```
let's add soem config
- Port range: limit to the ports we typically run local web apps on (one's you dont need high privilage for)
- List of filtered out process names; ["Browser", "ControlCE"] (in the UI add option to add a prcess to filter list)
```
[~2 min] Features added as expected, no errors

```
extend the readme as to how to install the build binary on the local computer (so it is alwasy avaialble)
```
[~1 min] README updated as expected I used `cargo install --path .`
