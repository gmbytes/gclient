extends RefCounted
class_name NetEventUtils

## Utility helpers (kept minimal — most logic now lives in handlers directly).

static func err_name(err_code: int) -> String:
	match err_code:
		0:  return "Ok"
		1:  return "Failed"
		3:  return "ServerNotFound"
		16: return "RoleNotFound"
		17: return "ServerMaintain"
		18: return "ServerBusy"
	return "Unknown(%d)" % err_code
