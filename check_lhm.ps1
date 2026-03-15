$r = (Invoke-WebRequest -Uri "http://127.0.0.1:8085/data.json" -UseBasicParsing).Content
$pattern = '"Text":"([^"]+)","Min":"[^"]*","Value":"([^"]*)","Max":"[^"]*","SensorId":"([^"]*)","Type":"Temperature"'
[regex]::Matches($r, $pattern) | ForEach-Object {
    "$($_.Groups[1].Value) = $($_.Groups[2].Value)  [$($_.Groups[3].Value)]"
}
