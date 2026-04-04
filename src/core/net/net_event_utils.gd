extends RefCounted
class_name NetEventUtils

## Unified field accessor: tries get_data() first, then get_extra().

static func field(event: NetEventGd, key: String, default = null):
	var d = event.get_data()
	if d:
		var v = d.get(key)
		if v != null:
			return v
	var ex: Dictionary = event.get_extra()
	if ex.has(key):
		return ex[key]
	return default


static func msg_err(event: NetEventGd) -> int:
	var v = field(event, "err", null)
	if v == null:
		return int(event.err)
	return int(v)
