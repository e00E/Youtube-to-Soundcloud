# Youtube to Soundcloud
This program exports Youtube playlists to Soundcloud playlists and is meant to run without user interaction. When run, it will copy the audio of new videos in the Youtube playlists to the corresponding Soundcloud playlists.

# Installation
If you are using the Windows release, install the [Microsoft Visual C++ Redistributable for Visual Studio 2017](https://go.microsoft.com/fwlink/?LinkId=746572).

Install [youtube-dl](https://rg3.github.io/youtube-dl/download.html). If you are using the Windows release, download `Windows_exe` via the previous link and put youtube-dl.exe in this application's folder. You also need the [Microsoft Visual C++ 2010 Redistributable Package (x86)](https://www.microsoft.com/en-US/download/details.aspx?id=5555) for youtube-dl to work.

Youtube-dl might require [FFmpeg](https://ffmpeg.org/download.html) to correctly process some videos. If you are using the Windows release, download FFmpeg (at the time of writing the current version is [here](http://ffmpeg.zeranoe.com/builds/win64/static/ffmpeg-3.3.2-win64-static.zip)) and put `ffmpeg.exe` in this application's folder.

# Configuration
The following files need to be edited before using the program:
* `config.json` contains general configuration options
* `playlists.json` contains the mapping of youtube playlists to soundcloud playlists

# Details
## config.json
* `soundcloud_client_id` is [your Soundcloud application](https://soundcloud.com/you/apps)'s client ID
* `soundcloud_client_secret` is your Soundcloud application's client secret
* `soundcloud_username` is your Soundcloud username. This is your login name / email address, not your display name.
* `soundcloud_password` is your Soundcloud password
* `youtube_api_key` is your Youtube api key

Ordinarily we would use oauth instead of username and password but that requires a domain and server while this application is meant to be run locally.

## playlists.json
* `playlists` is a list of playlists
* `youtube` is the ID of a youtube playlist
* `soundcloud` is the full url to a soundcloud playlist
* `position` is a positive integer which describes the index of the most recently transferred video in the youtube playlist

For new playlists, position should be set to 0 since Youtube playlists start at index 1. This application *only* considers the index to determine if a video needs to be moved to Soundcloud. This means that Youtube playlists need to have the oldest video at the lowest index and the newest video at the highest index.

If you were to for example set position to 5, then this application would start with the 6th video.

Once you have set up `playlists.json`, it will be updated automatically as this application completes audio exports, but you can still make manual changes if you want to.
