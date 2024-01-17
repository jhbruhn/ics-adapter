# ics-adapter

This is a tiny rust program which "proxies" .ics file URLs into a JSON format. It was developed to work with [OpenEPaperLink](https://github.com/jjwbruijn/OpenEPaperLink).

The usage is rather simple. It is recommended to run this as a Docker container. An example docker-compose configuration might look like this:

```yaml
version: "3"
services:
    calendar:
        image: ghcr.io/jhbruhn/ics-adapter:main
        ports:
            - "3000:3000"
        restart: unless-stopped
        environment:
            TZ: Europe/Berlin # your timezone!
            RULE_REPEATS: 100 # optional, the amount of repeats that should be done for repeating rules. Set to higher value if you are missing events of old repeating events
```

As you can see, you only have to forward port 3000 to wherever you like, in this case port 3000.
The URL you have to enter into the OEPL Google Calendar config then looks like this:

```
http://<docker-host>:3000/calendar/<ics-url>/entries
```

It is important that you urlencode the `<ics-url>` with a tool like this: https://www.urlencoder.org/

With an actual .ics url, it looks like this:
```
http://192.168.178.42:3000/calendar/https%3A%2F%2Fnextcloud.net%2Fremote.php%2Fdav%2Fpublic-calendars%2Fasdlkijf/entries
```

You may also list multiple urls to calendars separated with a `;`:
```
http://<docker-host>:3000/calendar/<ics-url-1>;<ics-url-2>/entries
```

Previous versions also supported these URL formats, but these are not compatible with modern OEPL features:

```
http://<docker-host>:3000/get?url=<ics-url>
```

With an actual .ics url, it looks like this:
```
http://192.168.178.42:3000/get?url=https://nextcloud.net/remote.php/dav/public-calendars/whateveryolo901i230ji
```

# License
MIT
