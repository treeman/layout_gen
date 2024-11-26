This is a keyboard layout visualization tool I made to create svgs I could embed into my blog.

See [this post](https://www.jonashietala.se/blog/2024/11/26/the_current_cybershard_layout/) for an example.

> [!important]
> I made this tool for my usage and mine alone. I don't have the time or energy to develop it as a general purpose tool and it will most likely break if you point it towards any other layout than my own.
> If you want to use it then I recommend you to fork it and tweak and fix it to your needs.

> [!caution]
> The code is in dire need for a refactor and is provided without documentation.
> It's a jungle out there.

Run with this for example:

```bash
cargo run --\
    --qmk-root ~/code/qmk_firmware\
    --keyboard cybershard\
    --render-opts render_settings.json\
    --output ~/code/jonashietala/images/cybershard-layout
```
