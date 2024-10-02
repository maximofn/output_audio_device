#!/usr/bin/bash
# Get script path
SCRIPT_PATH=$(dirname $0)
/usr/bin/python3 $SCRIPT_PATH/output_audio_device.py >output_audio_device.log 2>output_audio_device_error.log