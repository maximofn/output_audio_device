# Output audio device manager

🎤 Output audio device manager for Ubuntu: The Ultimate manager of audio devices. Manage what device is set as output device. This user-friendly and efficient application is fully integrated with the latest Ubuntu operating system. Get live updates and optimize your development tasks. Download now and take control of your output audio devices today!

![output audio devices manager](output_audio_device.gif)

## About audio device manager
Audio device manager is an intuitive tool designed everyone who need to change output audio device fast and easy. It integrates seamlessly with the Ubuntu menu bar, providing essential information at your fingertips.

## Key Features
 * Fast and easy: Change output audio device with a single click.
 * Time-saving: No need to navigate through the system settings.

## Installation

### Clone the repository

```bash
git clone https://github.com/maximofn/output_audio_device.git
```

or with `ssh`

```bash
git clone git@github.com:maximofn/output_audio_device.git
```

### Install the dependencies

Make sure that you do not have any `venv` or `conda` environment installed.

```bash
if [ -n "$VIRTUAL_ENV" ]; then
    deactivate
fi
if command -v conda &>/dev/null; then
    conda deactivate
fi
```

## Execution at start-up

```bash
add_to_startup.sh
```

Then when you restart your computer, the audio device manager will start automatically.

## Support

Consider giving a **☆ Star** to this repository, if you also want to invite me for a coffee, click on the following button

[![BuyMeACoffee](https://img.shields.io/badge/Buy_Me_A_Coffee-support_my_work-FFDD00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=white&labelColor=101010)](https://www.buymeacoffee.com/maximofn)