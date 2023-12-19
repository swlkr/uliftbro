htmx.defineExtension('json-enc', {
    onEvent: function (name, evt) {
        if (name === "htmx:configRequest") {
            evt.detail.headers['Content-Type'] = "application/json";
        }
    },
    
    encodeParameters : function(xhr, parameters, _elt) {
        xhr.overrideMimeType('text/json');
        let entries = Object.keys(parameters).map((k) => {
            if(isNumber(parameters[k])) {
                return [k, parseInt(parameters[k], 10)];
            } else {
                return [k, parameters[k]];
            }
        });
        let params = Object.fromEntries(entries);
        return (JSON.stringify(params));
    }
});

function isNumber(n) {
  return !isNaN(parseInt(n, 10)) && isFinite(n);
}
