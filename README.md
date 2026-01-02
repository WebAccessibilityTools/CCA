# Colour Contrast Analyser

This template should help get you started developing with Tauri in vanilla HTML, CSS and Typescript.

## Installation (MacOS)
Until I get money for Apple signing certificate

### Option 1. Disable The Gate Keeper (RECOMMENDED)
1. Download the latest release from the [Releases](https://github.com/WebAccessibilityTools/CCA/releases) page.

2. Move the unzipped `CCA.app` file to your Applications folder. **DO NOT DOUBLE CLICK.**

3. **Disable the Gate Keeper:** Open the Terminal app on your Mac and run the following command:
```shell
sudo spctl --master-disable
```

Choose `Anywhere` option under `System Settings`->`Privacy & Security`->`Security` section.

4. Double-click the `CCA.app` file to run it. 

5. You will be prompted with a warning that the app is from an unidentified developer. Click "Open".

<br/>

### Option 2 (Without disabling the Gate Keeper)
1. Download the latest release from the [Releases](https://github.com/WebAccessibilityTools/CCA/releases) page.

2. Since the app is not signed by Apple, your OS does not open the app.
You must enable `System Settings`->`Privacy & Security`->`Security`->`App Store and identified developers` option. 

3. Unzip the file. Double-click on icns Creator application file (`CCA.app`) to run it. It will not open because it is from an unidentified developer. Goto `System Settings`->`Privacy & Security`->`Security` and click `Open Anyway` button.
4. If prompted, allow the application to run on your system.
5. You're ready to start creating icns files out of PNG, JPG, or any other image document!

<br/>

### Option 3. Disable The Quarantine for CCA Only
1. Download the latest release from the [Releases](https://github.com/WebAccessibilityTools/CCA/releases) page.

2. Move the unzipped `CCA.app` file to your Applications folder. **DO NOT DOUBLE CLICK.**

3. **Quarantine for CCA:** Open the Terminal app on your Mac and run the following command:
```shell
sudo xattr -cr /Users/[user folder]/Applications/CCA.app
```

## Development
> pnpm install
> pnpm tauri dev

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)

## Test for ICC profiles
https://www.color-hex.com/