# Youtube to Soundcloud
This program exports Youtube playlists to Soundcloud playlists and is meant to run without user interaction. When run, it will copy the audio of new videos in the Youtube playlists to the corresponding Soundcloud playlists.

# Installation
Install [Python](https://www.python.org/downloads/). If you are using the release Windows binary, install a 64 bit version of Python. Make sure you tick the `add python to PATH` option during installation.

Run `pip install youtube-dl soundcloud six` to install required python dependencies.

Youtube-dl might require [FFmpeg](https://ffmpeg.org/download.html) to correctly process some videos. If you are using the release Windows binary, download ffmpeg and put ffmpeg.exe in this application's folder.

# Configuration
The following files need to be edited before using the program:
* `config.json` contains general configuration options
* `playlists.json` contains the mapping of youtube playlists to soundcloud playlists

# Details
## config.json
* `soundcloud_client_id` is [your soundcloud application](https://soundcloud.com/you/apps)'s client ID
* `soundcloud_client_secret` is your soundcloud application's client secret
* `soundcloud_username` is your soundcloud username. This is your login name / email address, not your display name.
* `soundcloud_password` is your soundcloud password
* `youtube_api_key` is your youtube api key

Ordinarily we would not use username and password directly and instead use oauth but that requires a domain and server while this application is meant to run locally.

## playlists.json
* `playlists` is a list of playlists
* `youtube` is the ID of a youtube playlist
* `soundcloud` is the full url to a soundcloud playlist
* `position` is a positive integer which describes the index of the most recently transferred video in the youtube playlist

For new playlists position should be set to 0 since youtube playlists start at index 1. This application *only* considers the index to determine if a video needs to be moved to Soundcloud. This means that youtube playlists need to have the oldest video at the lowest index and the newest video at the highest index.
