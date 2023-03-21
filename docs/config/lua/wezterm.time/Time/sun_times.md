# `Time:sun_times(lat, lon)`

{{since('20220807-113146-c2fee766')}}

For the date component of the time object, compute the times of the sun rise
and sun set for the given latitude and longitude.

For the time component of the time object, compute whether the sun is currently
up, and the progression of the sun through either the day or night.

Returns that information as a table:

```
> wezterm.time.now():sun_times(33.44, -112)
{
    "progression": 0.41843971631205673,
    "rise": "Time(utc: 2022-07-17T12:29:42.493449687+00:00)",
    "set": "Time(utc: 2022-07-18T02:36:40.776247739+00:00)",
    "up": true,
}
```

The example above computes the information for Phoenix, Arizona at the time
this documentation was being written.

The sun is presently up (`up == true`) and is about 41%
(`progression=0.41843971631205673`) of its way through the daylight portion of
the day.

If the sun would be down at the time portion of the time object, then `up ==
false` and the `progression` would indicate the proportion of the way through
the night.

This information is potentially useful if you want to vary color scheme or
other configuration based on the time of day.

If the provided latitude and longitude specify a location at one of the poles,
then the day or night may be longer than 24 hours. In that case `rise` and
`set` will be `nil`, `progression` will be `0` and `up` will indicate if it is
polar daytime (`up==true`) or polar night time (`up == false`).

