// 定义指标相关函数
// 依赖jquery, jquery-ui, d3

export const metric = {
    data,
    conf,
    draw,
    clear_drawing,
    outdate
};

// 指标显示类型
const _display = {
    "DIF": {
        "type": "line",
        "color": "gold",
        "enabled": true,
        "outdate": true,
        "status": "ok"
    },
    "DEA": {
        "type": "line",
        "color": "blue",
        "enabled": true,
        "outdate": true,
        "status": "ok"
    },
    "MACD": {
        "type": "bar",
        "enabled": true,
        "outdate": true,
        "status": "ok"
    }
};

const _draw_fn = {
    "line": draw_line,
    "bar": draw_bar
};

const _data = {
    "DIF": [],
    "DEA": [],
    "MACD": []
};

// metric的提示框
// 获取提示框，若不存在则创建
export function tooltip() {
    var t = d3.select("#m_container div.tooltip");
    if (!t.empty()) {
      return t;
    }
    return d3.select("#m_container")
      .append("div")
      .attr("class", "tooltip")
      .style("opacity", 0);
}

// 指标基础配置
function conf(metric_name) {
    // 单柱宽度，包含间隔
    var bar_width = parseFloat($("#bar_width").val());
    // 单柱间间隔
    var bar_padding;
    if ($("#bar_padding_fixed").is(":checked")) {
      bar_padding = parseFloat($("#bar_padding_fixed_width").val());
    } else {
      bar_padding = Math.max(bar_width / 3, 4);
    }
    // 柱内宽度，即显示出的红/绿柱宽度
    var bar_inner_width = bar_width - bar_padding;
    // 整体宽度
    var w = bar_width * _data[metric_name].length;
    // 整体高度
    var h = parseInt($("#metric_height").val());
    // 最大值
    var max_value = d3.max(_data[metric_name], function(d) {
      return d.value;
    });
    // 最小值
    var min_value = d3.min(_data[metric_name], function(d) {
        return d.value;
    });
    // 缩放比例
    var yscale = d3.scaleLinear([min_value, max_value], [0, h]);
    // 对于正负对称的图，使用对称缩放
    var abs_max = d3.max([Math.abs(max_value), Math.abs(min_value)]);
    var symscale = d3.scaleLinear([-abs_max, abs_max], [0, h]);
    return {
        metric_name,
        bar_width,
        bar_padding,
        bar_inner_width,
        w,
        h,
        yscale,
        symscale
    };
};

// data函数，刷新指定名称的指标数据
function data(metric_name, input) {
    if (input) {
        while(_data[metric_name].length > 0) { _data[metric_name].pop(); }
        for (var i = 0; i < input.length; i++) {
            // 将value转化为浮点数
            _data[metric_name].push({
                ts: input[i].ts,
                value: parseFloat(input[i].value)
            });
        }
        _display[metric_name].outdate = false;
        _display[metric_name].status = "ok";
        return;
    }
    return _data[metric_name];
}

function svg(conf) {
    if (conf === undefined) {
        return d3.select("#metric");
    }
    // 创建
    if (d3.select("#metric").empty()) {
        return d3.select("#m_container")
            .append("svg")
            .attr("id", "metric")
            .attr("width", conf.w)
            .attr("height", conf.h);
    }
    return d3.select("#metric");
}

function draw() {
    if (!d3.select("#metric").empty()) {
        // 如存在则删除
        d3.select("#metric").remove();
    }

    for (var mn of Object.keys(_display)) {
        if (_display[mn].enabled) {
            // 数据过期，则重新申请
            if (_display[mn].outdate) {
                // 直接返回
                return;
            }

            var conf = metric.conf(mn);
            for (var k of Object.keys(_display[mn])) {
                conf[k] = _display[mn][k];
            }
            // 调用画图方法
            _draw_fn[_display[mn].type](conf);
        }
    }
}

// draw_line函数
// conf={metric_name,color,w,h,symscale,bar_width}
function draw_line(conf) {
    var data = _data[conf.metric_name];
    // 确保svg存在
    svg(conf);

    var metric_selector = "#metric path." + conf.metric_name;
    if (!d3.select(metric_selector).empty()) {
        d3.select(metric_selector).remove();
    }
    // 画线
    var line = d3.line()
        .x(function(d, i) {
            return i * conf.bar_width + conf.bar_width / 2;
        })
        .y(function(d, i) {
            return conf.h - conf.symscale(d.value);
        });
    svg().append("path")
        .attr("class", conf.metric_name)
        .datum(data)
        .attr("d", line)
        .attr("stroke", conf.color)
        .attr("fill", "none");
};

// draw_bar函数
// conf={metric_name,color,w,h,symscale,bar_width,bar_padding,bar_inner_width}
function draw_bar(conf) {
    var data = _data[conf.metric_name];
    svg(conf);
    var metric_selector = "#metric rect." + conf.metric_name;
    if (!d3.selectAll(metric_selector).empty()) {
        d3.selectAll(metric_selector).remove();
    }
    // 画柱
    svg().selectAll("rect." + conf.metric_name).data(data).enter().append("rect")
        .attr("class", conf.metric_name)
        .attr("x", function(d, i) {
            return i * conf.bar_width;
        })
        .attr('y', function(d, i) {
            // 负数时高度为h/2
            var h = conf.symscale(d.value);
            if (h * 2 >= conf.h) {
                return conf.h - h;
            }
            return conf.h / 2;
        })
        .attr('width', conf.bar_inner_width)
        .attr("height", function(d) {
            // 最低高度1
            return Math.max(1, Math.abs(conf.h/2 - conf.symscale(d.value)));
        })
        .attr("fill", "none")
        .attr("stroke", function(d) {
            if (conf.symscale(d.value) * 2 >= conf.h) return "red";
            return "green";
        });
}

function clear_drawing() {
    // 删除浮动提示框
    if (!d3.select("#m_container div.tooltip").empty()) {
        d3.select("#m_container div.tooltip").remove();
    }
    // 删除指标图
    if (!d3.select("#metric").empty()) {
        d3.select("#metric").remove();
    }
}

function display(metric_name, enabled) {
    if (enabled === undefined) {
        return _display[metric_name].enabled;
    }
    _display[metric_name].enabled = enabled;
}

function outdate() {
    for (var mn of Object.keys(_display)) {
        _display[mn].outdate = true;
    }
}