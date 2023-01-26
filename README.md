# MP3 Duration Reporter

This tool iterates a directory for all files with the .mp3 extension, and
creates a file called "times.txt" in the parent of that directory with a list
of all the found files and their duration.

It also finally outputs a total duration to stdout after it's done running.

This tool may be useful for systems that need to manage and report on large
amounts of MP3 files.